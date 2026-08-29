//! Line-coverage measurement and the per-crate ratchet that drives it upward.
//!
//! ## Why a ratchet and not a single threshold
//!
//! A workspace-wide "must be ≥ N%" gate is the obvious design and the wrong one
//! here. This workspace is ~355k lines across ~127 first-party crates whose
//! testability varies by two orders of magnitude: `renzora_input`'s action map is
//! pure data transformation, while `renzora_ssao` is a render-graph node that
//! cannot execute at all without a GPU. One global number lets a well-tested
//! crate's gains silently pay for a regression somewhere else, and it has to be
//! set low enough for the worst crate, which means it gates nothing.
//!
//! So the floor is **per crate**, stored in `coverage-floors.txt`, and the rule is
//! only "never go down". A crate at 12% may not drop to 11%; a crate at 0% is
//! recorded as 0% and pinned there. `--bless` raises the floors after you have
//! added tests. That turns "100% coverage" from a cliff nobody can climb into a
//! monotone staircase — which is the only version of this goal that survives
//! contact with a codebase this size.
//!
//! ## Why the numbers are line coverage, and what they do not mean
//!
//! We measure LLVM source-based **line** coverage via `cargo-llvm-cov`, and we
//! measure it on the `dist` profile, because CLAUDE.md's one-profile rule is not
//! negotiable and because a `dev`-profile instrumented build of this workspace is
//! a second enormous artifact tree. `opt-level = 2` means the optimizer has
//! already merged and inlined code before instrumentation lands, so these
//! percentages run slightly *optimistic* versus an unoptimized build. They are a
//! trend line and a regression gate, not a proof of exhaustiveness — a 100%
//! covered function whose assertions are `assert!(true)` is still untested.
//!
//! `strip` is forced off for the run (see `cov_env`): the `dist` profile strips
//! symbols, and `llvm-cov` resolves its coverage records through the symbol
//! table, so an unmodified `dist` build reports every crate at 0% with no error.
//!
//! ## Run it on an otherwise idle machine
//!
//! Instrumentation shares nothing with the ordinary build, so a coverage run is
//! a second full compile of the workspace — and cargo will happily saturate
//! every core doing it. Started alongside a plain `cargo test`, the two together
//! exhausted RAM and the Windows pagefile here, and the way that surfaces is not
//! an out-of-memory message but a wall of nonsense:
//!
//! ```text
//! error: only metadata stub found for `dylib` dependency `std`
//! error[E0786]: found invalid metadata files for crate `renzora_tilemap_editor`
//!   = note: failed to mmap file '...rlib': The paging file is too small
//! ```
//!
//! Same shape as the full-disk failure in CLAUDE.md §2 — a compile error in a
//! crate nobody touched — and the same tell: it goes away on a re-run with the
//! machine to itself. CI hits none of this because the coverage job is the only
//! thing in its container.
//!
//! ## Two scopes, because the plugins are not workspace members
//!
//! `plugins/*` are deliberately excluded from the workspace (they must not
//! inherit the engine's feature unification and link Bevy). The cost is that
//! `cargo test --workspace` — and therefore CI, and therefore any workspace-wide
//! coverage run — has never once compiled them. That is ~14k lines of C-ABI
//! boundary code, the layer where a mistake is a hard crash in a user's game,
//! sitting entirely outside the gate. `scope = plugins` walks them individually.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Vendored third-party crates, excluded exactly as `.github/workflows/test.yml`
/// excludes them. Running their suites re-tests upstream against our Bevy fork
/// and breaks on every API drift; counting their lines would also bury the
/// first-party number under ~100k lines we do not own.
const VENDORED: &[&str] = &[
    "bevy_firefly",
    "bevy_gauge",
    "bevy_gaussian_splatting",
    "bevy_hanabi",
    "bevy_hui",
    "bevy_mod_outline",
    "bevy_silk",
    "vleue_navigator",
    "polyanya",
    "bvh2d",
    "bevy_mod_openxr",
    "bevy_mod_xr",
    "bevy_xr_utils",
];

/// Workspace members that are not desktop test targets: the mobile cdylib /
/// staticlib shells (no host test binary) and this helper itself.
const NON_TEST_MEMBERS: &[&str] = &["renzora-android", "renzora-ios", "xtask"];

/// Percentage points a blessed floor sits below the measured value, absorbing
/// cross-platform variance. See [`write_floors`] for why it is this wide.
const MARGIN: f64 = 1.5;

/// One crate's line-coverage tally, summed over every file that belongs to it.
#[derive(Default, Clone, Copy)]
struct Tally {
    found: u64,
    hit: u64,
}

impl Tally {
    fn percent(&self) -> f64 {
        if self.found == 0 {
            // A crate with no executable lines is vacuously covered. Reporting
            // 0% here would make `--check` demand tests for a crate that is
            // nothing but type definitions.
            100.0
        } else {
            self.hit as f64 * 100.0 / self.found as f64
        }
    }
}

pub fn run(repo: &Path, args: &[String]) -> ExitCode {
    let check = args.iter().any(|a| a == "--check");
    let bless = args.iter().any(|a| a == "--bless");
    let open = args.iter().any(|a| a == "--open");
    let skip_run = args.iter().any(|a| a == "--report-only");
    let plugins = args.iter().any(|a| a == "--plugins");
    let workspace = !plugins || args.iter().any(|a| a == "--workspace");

    if check && bless {
        eprintln!("[coverage] --check and --bless are opposites; pick one");
        return ExitCode::from(2);
    }

    if !skip_run && !have_llvm_cov() {
        eprintln!(
            "[coverage] cargo-llvm-cov is not installed. Install it with:\n\
             [coverage]   rustup component add llvm-tools-preview\n\
             [coverage]   cargo install cargo-llvm-cov --locked"
        );
        return ExitCode::FAILURE;
    }

    let out = repo.join("target").join("coverage");
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("[coverage] could not create {}: {e}", out.display());
        return ExitCode::FAILURE;
    }

    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();

    if workspace {
        let lcov = out.join("workspace.lcov");
        if !skip_run && !run_workspace(repo, &lcov, open) {
            return ExitCode::FAILURE;
        }
        match parse_lcov(repo, &lcov, Scope::Workspace) {
            Ok(t) => merge(&mut tallies, t),
            Err(e) => {
                eprintln!("[coverage] {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    if plugins {
        match run_plugins(repo, &out, skip_run) {
            Ok(t) => merge(&mut tallies, t),
            Err(e) => {
                eprintln!("[coverage] {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    if tallies.is_empty() {
        eprintln!(
            "[coverage] no coverage records were produced. If the run succeeded, \
             the usual cause is `strip` still being on for the profile — llvm-cov \
             resolves coverage through the symbol table."
        );
        return ExitCode::FAILURE;
    }

    print_table(&tallies);

    let floors_path = repo.join("coverage-floors.txt");
    if bless {
        return match write_floors(&floors_path, &tallies) {
            Ok(n) => {
                println!("[coverage] wrote {n} floors to {}", floors_path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("[coverage] {e}");
                ExitCode::FAILURE
            }
        };
    }
    if check {
        return check_floors(&floors_path, &tallies);
    }
    ExitCode::SUCCESS
}

// ── running the instrumented suites ──────────────────────────────────────────

fn have_llvm_cov() -> bool {
    Command::new(cargo())
        .args(["llvm-cov", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Environment every instrumented cargo invocation needs.
///
/// `STRIP=none` is the load-bearing one — see the module docs. `DEBUG=0` is a
/// disk concession: source-based coverage carries its own file/line records in
/// `__llvm_covmap`, so DWARF buys nothing here and this workspace's debuginfo is
/// measured in hundreds of gigabytes.
fn cov_env(cmd: &mut Command) {
    cmd.env("CARGO_PROFILE_DIST_STRIP", "none");
    cmd.env("CARGO_PROFILE_DIST_DEBUG", "0");
    cmd.env("CARGO_INCREMENTAL", "0");
}

fn run_workspace(repo: &Path, lcov: &Path, open: bool) -> bool {
    let mut args: Vec<String> = vec![
        "llvm-cov".into(),
        "--profile".into(),
        "dist".into(),
        "--workspace".into(),
        "--lcov".into(),
        "--output-path".into(),
        lcov.display().to_string(),
        // Keep dependency and vendored sources out of the report entirely.
        // Without this the registry checkouts of bevy/wgpu dominate the file
        // list and the first-party signal is unreadable.
        "--ignore-filename-regex".into(),
        ignore_regex(),
    ];
    for name in VENDORED.iter().chain(NON_TEST_MEMBERS) {
        args.push("--exclude".into());
        args.push((*name).into());
    }
    println!("[coverage] cargo {}", args.join(" "));
    let mut cmd = Command::new(cargo());
    cmd.current_dir(repo).args(&args);
    cov_env(&mut cmd);
    if !cmd.status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("[coverage] instrumented workspace run failed");
        return false;
    }

    // A second, cheap pass: the profraw data is already on disk, so re-rendering
    // it as HTML costs no compilation and no test time.
    let mut html = Command::new(cargo());
    html.current_dir(repo).args([
        "llvm-cov",
        "report",
        "--profile",
        "dist",
        "--html",
        "--ignore-filename-regex",
        &ignore_regex(),
    ]);
    cov_env(&mut html);
    let _ = html.status();
    if open {
        let mut o = Command::new(cargo());
        o.current_dir(repo)
            .args(["llvm-cov", "report", "--profile", "dist", "--open"]);
        cov_env(&mut o);
        let _ = o.status();
    }
    true
}

/// Walk `plugins/*` and run each one's suite under instrumentation.
///
/// Each plugin is its own cargo project with its own lockfile and target dir, so
/// there is no `--workspace` to lean on and no shared profraw directory to merge:
/// one invocation per plugin, one lcov file per plugin, summed here.
fn run_plugins(repo: &Path, out: &Path, skip_run: bool) -> Result<BTreeMap<String, Tally>, String> {
    let root = repo.join("plugins");
    let dir = out.join("plugins");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    let mut all = BTreeMap::new();
    let entries =
        std::fs::read_dir(&root).map_err(|e| format!("cannot read {}: {e}", root.display()))?;
    let mut names: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").exists())
        .collect();
    names.sort();

    for plugin in names {
        let name = file_name(&plugin);
        let lcov = dir.join(format!("{name}.lcov"));
        if !skip_run {
            println!("[coverage] cargo llvm-cov ({name})");
            let mut cmd = Command::new(cargo());
            cmd.current_dir(&plugin).args([
                "llvm-cov",
                "--profile",
                "dist",
                "--lcov",
                "--output-path",
                &lcov.display().to_string(),
            ]);
            cov_env(&mut cmd);
            if !cmd.status().map(|s| s.success()).unwrap_or(false) {
                return Err(format!("plugin '{name}' failed under instrumentation"));
            }
        }
        if lcov.exists() {
            merge(&mut all, parse_lcov(repo, &lcov, Scope::Plugin)?);
        }
    }
    Ok(all)
}

/// Files whose lines are not ours to cover: every dependency checkout, the
/// vendored Bevy-ecosystem crates, and generated code.
///
/// `crates/renzora_runtime/src/plugins.rs` and its editor twin are written by the
/// `add!` generator — they are a list of `add_plugins` calls with no branches, and
/// counting them would let a mechanical regeneration move the number.
fn ignore_regex() -> String {
    let mut alts: Vec<String> = vec![
        r"[/\\]\.cargo[/\\]registry[/\\]".into(),
        r"[/\\]rustc[/\\]".into(),
        r"[/\\]target[/\\]".into(),
        r"crates[/\\]bevy_oxr[/\\]".into(),
        r"renzora_runtime[/\\]src[/\\]plugins\.rs".into(),
        r"renzora_editor[/\\]src[/\\]plugins\.rs".into(),
        r"renzora_static_plugins[/\\]".into(),
    ];
    for v in VENDORED {
        alts.push(format!(r"crates[/\\]{v}[/\\]"));
    }
    alts.join("|")
}

// ── lcov parsing ─────────────────────────────────────────────────────────────

/// Which scope's lcov we are reading.
///
/// This is not cosmetic. Every plugin path-depends on `crates/renzora_plugin` for
/// the C-ABI headers, so a plugin's lcov carries `renzora_plugin` records too —
/// almost all of them zero-hit, because a plugin exercises the codec and not the
/// host loader. Summed naively across 69 plugins that buries `renzora_plugin`'s
/// real workspace coverage under 69 copies of its unhit lines. A plugin run only
/// ever contributes lines under `plugins/`.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Workspace,
    Plugin,
}

/// Sum an lcov file into per-crate tallies.
///
/// We only need three of lcov's record types: `SF:` opens a file, `LF:`/`LH:`
/// close it with lines-found and lines-hit. That is why this reads lcov rather
/// than llvm-cov's JSON — xtask is deliberately dependency-free (no serde), and
/// a line-oriented format needs no parser at all.
fn parse_lcov(repo: &Path, path: &Path, scope: Scope) -> Result<BTreeMap<String, Tally>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut out: BTreeMap<String, Tally> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(file) = line.strip_prefix("SF:") {
            current = crate_of(repo, file).filter(|name| match scope {
                Scope::Workspace => !name.starts_with("plugins/"),
                Scope::Plugin => name.starts_with("plugins/"),
            });
        } else if let Some(n) = line.strip_prefix("LF:") {
            if let (Some(name), Ok(n)) = (current.as_ref(), n.trim().parse::<u64>()) {
                out.entry(name.clone()).or_default().found += n;
            }
        } else if let Some(n) = line.strip_prefix("LH:") {
            if let (Some(name), Ok(n)) = (current.as_ref(), n.trim().parse::<u64>()) {
                out.entry(name.clone()).or_default().hit += n;
            }
        } else if line == "end_of_record" {
            current = None;
        }
    }
    Ok(out)
}

/// Map a source path to the crate that owns it.
///
/// Nested editor subcrates (`crates/renzora_engine/editor/`) are reported under
/// their own package name, `renzora_engine_editor`, because that is what cargo
/// calls them and what a floor line has to name. Plugins get a `plugins/` prefix
/// so a plugin and a crate of the same name cannot collide in the floor file.
fn crate_of(repo: &Path, file: &str) -> Option<String> {
    let norm = file.replace('\\', "/");
    let root = repo.display().to_string().replace('\\', "/");
    let rel = norm
        .strip_prefix(&format!("{root}/"))
        .unwrap_or(&norm)
        .to_string();

    let mut parts = rel.split('/');
    match parts.next()? {
        "crates" => {
            let name = parts.next()?;
            // `crates/<name>/editor/src/...` is a separate package.
            if parts.next() == Some("editor") {
                Some(format!("{name}_editor"))
            } else {
                Some(name.to_string())
            }
        }
        "plugins" => Some(format!("plugins/{}", parts.next()?)),
        // The root `renzora_app` binary crate lives at `src/` in the repo root.
        "src" => Some("renzora_app".to_string()),
        _ => None,
    }
}

fn merge(into: &mut BTreeMap<String, Tally>, from: BTreeMap<String, Tally>) {
    for (k, v) in from {
        let e = into.entry(k).or_default();
        e.found += v.found;
        e.hit += v.hit;
    }
}

// ── reporting and the ratchet ────────────────────────────────────────────────

fn print_table(tallies: &BTreeMap<String, Tally>) {
    let mut rows: Vec<(&String, &Tally)> = tallies.iter().collect();
    // Worst first: the top of this list is the next work to do.
    rows.sort_by(|a, b| {
        a.1.percent()
            .partial_cmp(&b.1.percent())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.found.cmp(&a.1.found))
    });

    println!("\n{:<44} {:>7} {:>9} {:>9}", "CRATE", "COV%", "COVERED", "LINES");
    for (name, t) in &rows {
        println!(
            "{:<44} {:>6.1}% {:>9} {:>9}",
            name,
            t.percent(),
            t.hit,
            t.found
        );
    }
    let total = rows.iter().fold(Tally::default(), |mut a, (_, t)| {
        a.found += t.found;
        a.hit += t.hit;
        a
    });
    println!(
        "\n[coverage] TOTAL {:.2}%  ({} / {} lines across {} crates)",
        total.percent(),
        total.hit,
        total.found,
        rows.len()
    );
}

fn write_floors(path: &Path, tallies: &BTreeMap<String, Tally>) -> Result<usize, String> {
    let mut s = String::from(
        "# Per-crate line-coverage floors. Generated by `cargo renzora coverage --bless`.\n\
         #\n\
         # `cargo renzora coverage --check` fails if any crate falls below its floor,\n\
         # so this file only ever moves up. Raise it in the same commit that adds the\n\
         # tests; never lower a line to make CI green.\n\
         #\n\
         # Format: <package> <floor-percent>\n",
    );
    for (name, t) in tallies {
        // Floor below the measured value, because coverage is not
        // bit-reproducible across platforms: a `#[cfg(windows)]` branch is an
        // uncovered line on Linux and vice versa, so a floor pinned to the exact
        // reading fails CI on the other host for no behavioural reason.
        //
        // The margin was 0.5 and that was measured to be too small. `renzora`
        // blessed at 39.0 from a Windows run and CI's Linux run reported 38.8 —
        // a 0.7-point gap, because the contract crate carries a lot of
        // platform-gated path handling. 1.5 covers the widest gap seen so far.
        //
        // The margin is a workaround for blessing on the wrong host, not a
        // substitute for it: bless from a Linux measurement (see `--report-only`
        // and the CI artifact recipe in the testing docs) and the skew is zero.
        let floor = (t.percent() - MARGIN).max(0.0);
        s.push_str(&format!("{name} {floor:.1}\n"));
    }
    std::fs::write(path, &s).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(tallies.len())
}

fn check_floors(path: &Path, tallies: &BTreeMap<String, Tally>) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!(
                "[coverage] no {} yet — run `cargo renzora coverage --bless` to \
                 record the current numbers as the starting floor.",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let mut floors: BTreeMap<&str, f64> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        if let (Some(name), Some(pct)) = (it.next(), it.next()) {
            if let Ok(pct) = pct.parse::<f64>() {
                floors.insert(name, pct);
            }
        }
    }

    let mut failed = 0;
    for (name, t) in tallies {
        let Some(floor) = floors.get(name.as_str()) else {
            // A new crate with no floor is not a failure — it is a crate whose
            // floor has not been blessed yet. Say so loudly rather than gating
            // on a number nobody chose.
            println!("[coverage] NEW  {name} at {:.1}% (no floor recorded)", t.percent());
            continue;
        };
        if t.percent() + 1e-9 < *floor {
            eprintln!(
                "[coverage] DROP {name}: {:.1}% is below its floor of {floor:.1}%",
                t.percent()
            );
            failed += 1;
        }
    }

    // A crate that vanished from the report is more often a build that silently
    // stopped producing coverage for it than a deleted crate, and that failure
    // mode reads as "coverage held steady" if we ignore it.
    for name in floors.keys() {
        if !tallies.contains_key(*name) {
            println!("[coverage] GONE {name} produced no coverage records this run");
        }
    }

    if failed > 0 {
        eprintln!(
            "\n[coverage] {failed} crate(s) regressed. Add tests, or if the drop is \
             genuinely correct (dead code deleted, crate merged), re-bless with \
             `cargo renzora coverage --bless` and say why in the commit."
        );
        return ExitCode::FAILURE;
    }
    println!("[coverage] all crates at or above their floors");
    ExitCode::SUCCESS
}

// ── local copies of main.rs's helpers ────────────────────────────────────────

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
