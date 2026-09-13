//! Build a directory of native plugins against a staged SDK and report on it.
//!
//! This is the release gate for <https://github.com/renzora/plugins>: it answers
//! "does the engine about to ship still compile the plugins people have
//! installed", which is a question nothing else in CI asks. A native plugin is
//! shipped as source and compiled on the machine that installs it, so an engine
//! change that breaks one is invisible until a user launches an editor and finds
//! the plugin skipped.
//!
//! # Why an example rather than a crate or an xtask subcommand
//!
//! An `examples/` target is not built by `cargo build --workspace`, so this costs
//! nothing on every other build, and it needs no new workspace member.
//!
//! xtask would have been the obvious home, and `CLAUDE.md` still lists a
//! `xtask/src/native_plugin.rs` that was never written. It is the wrong home:
//! xtask is deliberately its own workspace root so it keeps building when a
//! dangling generated dependency stops the engine workspace from loading, which
//! is what makes `cargo renzora sync` able to repair that state. A path
//! dependency on an engine crate puts that property at risk, because resolving
//! this crate's `lints.workspace = true` means loading the very workspace that
//! is broken. The repair tool must not depend on the thing it repairs.
//!
//! # What it proves, and what it does not
//!
//! It compiles. That catches the failure that actually happens: a contract-crate
//! API change that a plugin's source no longer matches. It does NOT load the
//! result, so a plugin that compiles and then panics in `Plugin::build` passes
//! here. Loading is worth adding, and has to run one plugin per subprocess: the
//! loader never drops a `Library` (every function pointer a plugin registered
//! points into that image), so 74 of them in one process only accumulates.
//!
//! # Usage
//!
//! ```text
//! cargo run -p renzora_plugin_build --profile dist --example check_plugins -- \
//!     --sdk dist/windows-x64/sdk \
//!     --plugins ../plugins \
//!     --summary $GITHUB_STEP_SUMMARY
//! ```
//!
//! Exits 0 even when plugins fail, because the CI step that runs it sits inside
//! a desktop lane that has already spent hours building. Failing there would
//! drop a whole platform out of the release over one broken third-party plugin,
//! which is a worse outcome than a red line in a report. Pass `--strict` to make
//! failures fatal, which is what a workflow in the plugins repo itself wants.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use renzora_plugin_build::Sdk;

/// The symbol `renzora::plugin!` writes and the loader searches for.
///
/// Checked here the same way the loader checks it, by scanning the built image
/// for the bytes rather than by loading it: a library without this is not a
/// plugin, and the loader skips it silently. A plugin that compiles but does not
/// export it would therefore be installed and do nothing, which is exactly the
/// failure this catches and a compile-only check otherwise would not.
const CTOR: &[u8] = b"renzora_native_plugin_ctor";

struct Outcome {
    name: String,
    ok: bool,
    detail: String,
    secs: f32,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let strict = argv.iter().any(|a| a == "--strict");
    let sdk_dir = match flag(&argv, "--sdk") {
        Some(v) => PathBuf::from(v),
        None => {
            eprintln!("usage: check_plugins --sdk <dir> --plugins <dir> [--summary <file>] [--strict]");
            return ExitCode::from(2);
        }
    };
    let plugins_dir = match flag(&argv, "--plugins") {
        Some(v) => PathBuf::from(v),
        None => {
            eprintln!("usage: check_plugins --sdk <dir> --plugins <dir> [--summary <file>] [--strict]");
            return ExitCode::from(2);
        }
    };

    let sdk = match Sdk::load(&sdk_dir) {
        Ok(s) => s,
        Err(e) => {
            // Not a plugin failure, so it is fatal even in report mode: a run
            // that silently checked nothing is worse than one that stopped.
            eprintln!("could not load the SDK at {}: {e}", sdk_dir.display());
            return ExitCode::FAILURE;
        }
    };
    let triple = sdk.manifest().triple.clone();
    let ext = sdk.manifest().lib_ext.clone();

    let dirs = match plugin_dirs(&plugins_dir) {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => {
            eprintln!("no plugin directories under {}", plugins_dir.display());
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("could not read {}: {e}", plugins_dir.display());
            return ExitCode::FAILURE;
        }
    };

    println!("checking {} plugins against {triple}\n", dirs.len());

    let build_root = std::env::temp_dir().join(format!("renzora-plugin-check-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&build_root);

    let mut results = Vec::new();
    for dir in &dirs {
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let out = build_root.join(format!("{}.{ext}", name.replace('-', "_")));
        let started = Instant::now();
        let outcome = match sdk.compile(dir, &out) {
            Ok(_) if !exports_ctor(&out) => (false, format!("built but exports no {}", String::from_utf8_lossy(CTOR))),
            Ok(_) => (true, String::new()),
            // Only the first line. rustc's full diagnostic is worth keeping for
            // one failure and unreadable for twenty, and the log above this
            // already carries it.
            Err(e) => (false, first_line(&e.to_string())),
        };
        let r = Outcome {
            name,
            ok: outcome.0,
            detail: outcome.1,
            secs: started.elapsed().as_secs_f32(),
        };
        println!(
            "  {} {:<28} {:>6.1}s  {}",
            if r.ok { "ok  " } else { "FAIL" },
            r.name,
            r.secs,
            r.detail
        );
        results.push(r);
    }
    let _ = std::fs::remove_dir_all(&build_root);

    let failed: Vec<&Outcome> = results.iter().filter(|r| !r.ok).collect();
    println!(
        "\n{} of {} built against {triple}",
        results.len() - failed.len(),
        results.len()
    );
    for r in &failed {
        println!("  FAIL {}: {}", r.name, r.detail);
    }

    if let Some(path) = flag(&argv, "--summary") {
        if let Err(e) = write_summary(Path::new(&path), &triple, &results) {
            // A summary that could not be written must not change the verdict.
            eprintln!("could not write the summary to {path}: {e}");
        }
    }

    if strict && !failed.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Every immediate subdirectory holding a plugin, sorted so two runs of the same
/// tree report in the same order.
///
/// `src/lib.rs` is what makes a directory a plugin, exactly as the loader has it.
/// Matching on that rather than on the manifest skips the repo's own `.github`,
/// docs and asset directories without needing a list of names to ignore.
fn plugin_dirs(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("src").join("lib.rs").is_file() && p.join("Cargo.toml").is_file())
        .collect();
    out.sort();
    Ok(out)
}

/// Does the built library export the constructor, by byte search?
///
/// The loader decides what is a plugin this way, before it maps the image, so
/// this asks the same question the same way rather than a more thorough one that
/// could disagree with it.
fn exports_ctor(lib: &Path) -> bool {
    let Ok(bytes) = std::fs::read(lib) else { return false };
    bytes.windows(CTOR.len()).any(|w| w == CTOR)
}

fn first_line(s: &str) -> String {
    s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

fn flag(argv: &[String], name: &str) -> Option<String> {
    let i = argv.iter().position(|a| a == name)?;
    argv.get(i + 1).cloned()
}

/// A markdown table for `$GITHUB_STEP_SUMMARY`.
///
/// Appended rather than written, because every desktop lane writes to the same
/// per-job file and a release run has six of them.
fn write_summary(path: &Path, triple: &str, results: &[Outcome]) -> std::io::Result<()> {
    use std::io::Write as _;

    let failed: Vec<&Outcome> = results.iter().filter(|r| !r.ok).collect();
    let mut s = String::new();
    let _ = writeln!(
        s,
        "### Plugins on `{triple}`\n\n**{} of {} built.**\n",
        results.len() - failed.len(),
        results.len()
    );
    if failed.is_empty() {
        let _ = writeln!(s, "Every plugin compiled and exports its constructor.\n");
    } else {
        let _ = writeln!(s, "| Plugin | Why it failed |\n|---|---|");
        for r in &failed {
            // Pipes would break the table, and a rustc note can contain them.
            let _ = writeln!(s, "| `{}` | {} |", r.name, r.detail.replace('|', "\\|"));
        }
        let _ = writeln!(s);
    }

    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(s.as_bytes())
}
