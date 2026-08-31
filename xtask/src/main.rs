//! `cargo renzora` — native build + stage + run, the local mirror of Docker.
//!
//! Docker gives a contributor two things: a pinned toolchain (so everyone
//! compiles with the same rustc) and a build that arranges the output into a
//! runnable `dist/` layout. The toolchain pin is handled by `rust-toolchain.toml`
//! (rustup auto-selects 1.95.0). This binary handles the second half WITHOUT a
//! container, for the host platform only:
//!
//!   1. `cargo build --profile dist --workspace` — compile BOTH executables
//!      (`renzora`, the runtime/shipped game, and `renzora-editor`) plus every
//!      distribution plugin cdylib. One invocation: the ~700 shared crates
//!      compile once and are linked into both binaries.
//!   2. Stage `dist/<platform>/`: the two executables + the shared libraries +
//!      the OpenXR loader beside each other, every plugin cdylib into `plugins/`.
//!   3. (`run` only) launch the staged editor.
//!
//! ## The shared libraries
//!
//! `renzora_app`'s default features include `dynamic_linking`, so Bevy lives in
//! one `bevy_dylib` that both executables import rather than a private copy
//! baked into each. Measured on Windows, that took the pair from 460 MB to
//! 397 MB, and it takes relinking the whole of Bevy out of every build.
//!
//! It is also the prerequisite for a plugin ever holding `&mut World`: Rust
//! derives a type's `TypeId` from how it was compiled, so two independently
//! linked copies of Bevy disagree about what `Transform` is. One shared image
//! means one answer.
//!
//! Two files therefore have to land beside the executables, and a missing one
//! is not a clean error — the OS loader refuses the binary before `main`:
//!
//!   * `bevy_dylib.<ext>`, from `target/dist/`.
//!   * `std-<hash>.<ext>`, from the rustc sysroot. This one is easy to forget
//!     because nothing in the workspace asks for it: linking *any* dylib makes
//!     rustc link std dynamically too, so it arrives as a side effect of
//!     `dynamic_linking` rather than of `prefer-dynamic` (which this repo does
//!     not set). The filename hashes the toolchain, so it must be re-staged
//!     whenever `rust-toolchain.toml` moves.
//!
//! The editor is still a second *executable* rather than the removable
//! `renzora_editor.dll` it used to be. That is unrelated to the above and has
//! not changed: `renzora_viewport::external_runtime` spawns `renzora` as a child
//! process for play mode, so both binaries must be staged together regardless.
//!
//! Why a staging step at all: a bare `cargo run` leaves the plugin cdylibs flat
//! in `target/dist/` next to the exe, but the dynamic loader scans
//! `<exe-dir>/plugins/` — so those plugins compile but never load. Step 2 is the
//! one thing `cargo run` can't do, and the only reason `cargo renzora` exists.
//!
//! Cross-platform builds stay Docker-only (`renzora build` / `build-all.sh`):
//! this tool only ever produces artifacts for the machine it runs on.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod bundle;
mod coverage;
mod native_plugin;
mod sdk;
mod sync;
mod wasm;

/// Host-platform naming. Filled at compile time from `cfg!` because xtask is
/// built for — and run on — the very platform it stages for.
pub(crate) struct Platform {
    /// `dist/<dir>` — matches `build-all.sh`'s platform directory names.
    dir: &'static str,
    /// Shared-library extension, no dot (`dll` / `so` / `dylib`).
    pub(crate) ext: &'static str,
    /// `lib` on Unix, empty on Windows — the Cargo dylib filename prefix.
    lib_prefix: &'static str,
    /// `.exe` on Windows, empty elsewhere.
    exe_suffix: &'static str,
}

fn platform() -> Platform {
    // Arch only distinguishes the dist dir name (x64 vs arm64); the binaries are
    // always native, so we never cross-target here.
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(target_os = "windows") {
        Platform {
            dir: if arm { "windows-arm64" } else { "windows-x64" },
            ext: "dll",
            lib_prefix: "",
            exe_suffix: ".exe",
        }
    } else if cfg!(target_os = "macos") {
        Platform {
            dir: if arm { "macos-arm64" } else { "macos-x64" },
            ext: "dylib",
            lib_prefix: "lib",
            exe_suffix: "",
        }
    } else {
        Platform {
            dir: if arm { "linux-arm64" } else { "linux-x64" },
            ext: "so",
            lib_prefix: "lib",
            exe_suffix: "",
        }
    }
}

fn main() -> ExitCode {
    // Resolve the workspace root from this crate's manifest dir (`<root>/xtask`)
    // so `cargo renzora` works regardless of the caller's cwd.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at <repo>/xtask")
        .to_path_buf();

    let cmd = std::env::args().nth(1).unwrap_or_else(|| "run".to_string());
    let plat = platform();

    match cmd.as_str() {
        // Build + stage + launch — the default `cargo renzora`.
        //
        // **Flat, pipelined boot.** This used to launch without `RENZORA_NO_XR`,
        // which meant a dev with an OpenXR runtime *installed and set as the
        // system default* — not connected, not in use, merely present — got the
        // XR-capable editor boot. That disables `PipelinedRenderingPlugin`, so the
        // render sub-app runs inline on the main thread instead of in parallel
        // with the sim: measured at ~11.6 ms of a 27 ms frame. The symptom that
        // found it was `cargo renzora profile` being *faster* than `cargo renzora`,
        // because only the profiling lane was passing the opt-out.
        //
        // Editing in VR is `cargo renzora xr`; shipping a VR game is unaffected
        // (the runtime binary's `--vr` path is separate).
        "run" => {
            let out = match build_and_stage(&repo, &plat, &[]) {
                Ok(out) => out,
                Err(code) => return code,
            };
            launch(&repo, &out, &plat, true)
        }
        // Build + stage + launch the XR-capable editor.
        //
        // The explicit opt-in half of the change described on `run`. Boots with
        // the XR plugins and *without* pipelined rendering, which is what the
        // headset compositor needs (it wants synchronous submission). Expect a
        // lower flat-screen frame rate — that is inherent to the boot, not a
        // regression.
        "xr" => {
            let out = match build_and_stage(&repo, &plat, &[]) {
                Ok(out) => out,
                Err(code) => return code,
            };
            launch(&repo, &out, &plat, false)
        }
        // Build + stage only — produce the dist/ folder, don't launch.
        //
        // `--bundle` additionally wraps the result in the shape that ships: an
        // AppImage on Linux, a `.app` on macOS, nothing on Windows (where a flat
        // folder already is the shipping layout). Opt-in because wrapping moves
        // the binary out from under the launch step and makes every local build
        // pay for `mksquashfs`; the CI desktop lanes ask for it, a contributor
        // iterating does not. See `bundle.rs` for why it has to exist at all.
        "dist" => match build_and_stage(&repo, &plat, &[]) {
            Ok(out) => {
                if std::env::args().any(|a| a == "--bundle") {
                    if let Err(e) = bundle::wrap(&repo, &out, &plat) {
                        eprintln!("[xtask] error: could not bundle {}: {e}", out.display());
                        return ExitCode::FAILURE;
                    }
                }
                println!("[xtask] staged {}", out.display());
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        // Regenerate ONLY the plugin SDK, without rebuilding or restaging
        // anything else. Every staged build already refreshes it (see
        // `build_and_stage`); this is for when that is all you want to redo.
        // Optional flags exist for `docker/build-all.sh`, which stages every
        // platform from one checkout and therefore cannot use any of the
        // defaults: `--target-dir target/editor`, `--target <triple>` for a cross
        // build, and `--out` because the container's layout is its own.
        //
        // Without these the container had no way to stage an SDK at all, so
        // every published release shipped without one — the archive step in
        // `package-release.sh` found no `sdk/` and silently produced nothing.
        "sdk" => {
            let argv: Vec<String> = std::env::args().skip(2).collect();
            let from = sdk::From {
                target_dir: flag_value(&argv, "--target-dir"),
                target: flag_value(&argv, "--target"),
            };
            let out_dir = flag_value(&argv, "--out")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo.join("dist").join(plat.dir));
            match sdk::build_from(&repo, &plat, &out_dir, &from) {
                Ok(out) => {
                    println!("[xtask] staged {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(code) => code,
            }
        }
        // Profiling build + stage + launch — same as `run` but compiles the
        // `profiling` feature in, which re-adds Bevy's Tracy instrumentation
        // (per-system + render-node CPU zones, GPU-pass zones, frame marks). Use
        // it with a running Tracy server to get the full flame graph + per-system
        // Statistics. It recompiles `bevy_dylib` with `trace_tracy`, so prebuilt
        // community plugins won't load against it — everything built here from
        // source still matches (CLAUDE.md §3).
        //
        // Launches with `RENZORA_NO_XR=1` unless you pass `--xr`. A dev with an
        // OpenXR runtime installed and set as the system default gets the
        // XR-capable editor boot, which disables `PipelinedRenderingPlugin` and so
        // runs the render sub-app inline on the main thread — measured at ~11.6 ms
        // of a 27 ms frame, i.e. the profile is dominated by a serialization you
        // almost certainly didn't mean to measure. Pass `--xr` when the headset
        // path is the thing under the microscope.
        "profile" => {
            let out = match build_and_stage(&repo, &plat, &["profiling"]) {
                Ok(out) => out,
                Err(code) => return code,
            };
            launch(&repo, &out, &plat, true)
        }
        // Build ONE standalone plugin and stage just its library — the hot-reload
        // loop.
        //
        // Separate from `dist` because `dist` copies `renzora.exe`, and a running
        // editor holds that file open on Windows: a full stage always requires
        // closing the editor, which is exactly what hot reload exists to avoid.
        // Staging one plugin touches nothing the editor has locked, because the
        // loader maps a copy under `plugins/.reload/` and leaves the original free.
        "plugin" => {
            let Some(name) = std::env::args().nth(2) else {
                eprintln!("[xtask] usage: cargo renzora plugin <name>");
                return ExitCode::from(2);
            };
            stage_one_plugin(&repo, &plat, &name)
        }
        // Delete a plugin crate and every reference to it, in one process — the
        // only safe way to do it. See `sync::remove`.
        "remove" => {
            let Some(name) = std::env::args().nth(2) else {
                eprintln!("[xtask] usage: cargo renzora remove <crate-name>");
                return ExitCode::from(2);
            };
            match sync::remove(&repo, &name) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("[xtask] {e}");
                    ExitCode::FAILURE
                }
            }
        }
        // Regenerate the plugin wiring from the `renzora::add!` declarations
        // under `crates/`. Runs automatically before every build; standalone
        // here for the case where you want the diff without a compile, and with
        // `--check` for CI, which must fail if a declaration was added without
        // committing the regenerated files.
        "sync" => {
            let check = std::env::args().any(|a| a == "--check");
            match sync::sync(&repo, check) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("[xtask] {e}");
                    ExitCode::FAILURE
                }
            }
        }
        // Measure line coverage and enforce the per-crate ratchet in
        // `coverage-floors.txt`. See `coverage.rs` for why the gate is per-crate
        // and never a single workspace threshold.
        //
        //   cargo renzora coverage                  workspace, print the table
        //   cargo renzora coverage --plugins        the C-ABI plugins too
        //   cargo renzora coverage --check          fail if any crate regressed
        //   cargo renzora coverage --bless          record the current numbers
        //   cargo renzora coverage --report-only    re-read the last run's lcov
        //
        // This is the one command that deliberately builds outside
        // `target/dist`: instrumentation changes the fingerprint of every crate,
        // so cargo-llvm-cov keeps its artifacts in `target/llvm-cov-target/`.
        // That tree is disposable — delete it when you need the disk back.
        "coverage" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            coverage::run(&repo, &args)
        }
        // Build + stage the two WEB bundles into `dist/web-wasm32/`, the same
        // place and layout the container's `build_wasm` lane produces.
        //
        // The one `cargo renzora` command that cross-compiles — every other
        // builds for the host. Nothing links a host artefact here, so that costs
        // nothing; the point of the target is that it runs in a browser.
        "wasm" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            wasm::build_and_stage(&repo, &args)
        }
        other => {
            eprintln!(
                "[xtask] unknown command '{other}' \
                 (expected: run | xr | dist | sdk | wasm [--no-opt] | plugin <name> | profile | \
                 coverage [--check|--bless] | sync [--check] | remove <crate-name>)"
            );
            ExitCode::from(2)
        }
    }
}

/// Build `plugins/<name>` and copy its library into the staged `plugins/`.
///
/// The editor's watcher notices the changed file and reloads it, so this is the
/// whole edit-build-see loop — no relaunch, and no contention with the running
/// process.
fn stage_one_plugin(repo: &Path, plat: &Platform, name: &str) -> ExitCode {
    let dir = repo.join("plugins").join(name);
    if !dir.join("Cargo.toml").exists() {
        eprintln!("[xtask] no plugin at {}", dir.display());
        return ExitCode::FAILURE;
    }
    println!("[xtask] cargo build --profile dist ({name})");
    let built = Command::new(cargo())
        .current_dir(&dir)
        .args(["build", "--profile", "dist"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !built {
        return ExitCode::FAILURE;
    }

    let out = repo.join("dist").join(plat.dir).join("plugins");
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("[xtask] could not create {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    // All plugins share `plugins/target/` (see `plugins/.cargo/config.toml`), so
    // this directory holds every plugin's artifact, not just this one's. Resolve
    // the package name from the manifest — a crate's library name is not always
    // its directory name — and copy only that file, or a single-plugin rebuild
    // would restage all 61.
    let from = repo.join("plugins").join("target").join("dist");
    let pkg = std::fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.trim_start().starts_with("name = "))
                .and_then(|l| l.split('"').nth(1).map(|s| s.replace('-', "_")))
        })
        .unwrap_or_else(|| name.replace('-', "_"));
    let wanted = format!("{}{}.{}", plat.lib_prefix, pkg, plat.ext);
    let Ok(entries) = std::fs::read_dir(&from) else {
        eprintln!("[xtask] nothing built at {}", from.display());
        return ExitCode::FAILURE;
    };
    let mut staged = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || file_name(&path) != wanted {
            continue;
        }
        if let Err(e) = copy(&path, &out.join(file_name(&path))) {
            eprintln!("[xtask] {e}");
            return ExitCode::FAILURE;
        }
        println!("[xtask] staged {}", out.join(file_name(&path)).display());
        staged += 1;
    }
    if staged == 0 {
        eprintln!("[xtask] {name} built but produced no {wanted} — is it a cdylib?");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn build_and_stage(repo: &Path, plat: &Platform, features: &[&str]) -> Result<PathBuf, ExitCode> {
    // Wire in any plugin crate whose `renzora::add!` declaration isn't reflected
    // in the generated lists yet — this is what makes "drop a crate into
    // `crates/`" the whole job. Cheap (a source scan) and a no-op when nothing
    // moved, so it runs before every build rather than being something to
    // remember.
    if let Err(e) = sync::sync(repo, false) {
        eprintln!("[xtask] plugin sync failed: {e}");
        return Err(ExitCode::FAILURE);
    }
    if !build(repo, features) {
        eprintln!("[xtask] cargo build failed");
        return Err(ExitCode::FAILURE);
    }
    match stage(repo, plat) {
        Ok(out) => {
            // The plugin SDK, beside the binaries it was cut from. An SDK only
            // works against its own engine build — every metadata filename
            // hashes the build configuration — so regenerating it here is what
            // stops a stale one from ever sitting next to a fresh editor. It hardlinks,
            // so it costs neither disk nor noticeable time (see `sdk.rs`).
            sdk::build(repo, plat, &out)?;
            // After the SDK, because they link against it. A native plugin in
            // `plugins/` is built exactly the way a user's installed one is —
            // which is the point: an author working from source exercises the
            // real path rather than a dev-only shortcut.
            if !native_plugin::build_all(repo, &out) {
                return Err(ExitCode::FAILURE);
            }
            Ok(out)
        }
        Err(e) => {
            eprintln!("[xtask] staging failed: {e}");
            // On Windows this is almost always one thing, and the OS error says
            // nothing about which process. Guessing costs minutes; saying so costs
            // a line.
            if e.kind() == std::io::ErrorKind::PermissionDenied
                || e.raw_os_error() == Some(32)
            {
                eprintln!(
                    "[xtask] a file in dist/ is open in another process — usually a \
                     running editor holding renzora.exe. Close it and re-run.\n\
                     [xtask] to reload just a plugin WITHOUT closing the editor: \
                     cargo renzora plugin <name>"
                );
            }
            Err(ExitCode::FAILURE)
        }
    }
}

/// Compile the workspace exactly as the container's editor lane does
/// (`build-all.sh`): the whole workspace on the `dist` profile, minus the
/// mobile crates (cdylib/staticlib targets that don't belong in a desktop
/// build) and minus this helper itself.
fn build(repo: &Path, features: &[&str]) -> bool {
    let profile = profile();
    let mut args: Vec<String> = [
        "build",
        "--profile",
        &profile,
        "--workspace",
        "--exclude",
        "renzora-android",
        "--exclude",
        "renzora-ios",
        "--exclude",
        "xtask",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // Enable the requested workspace features (e.g. `profiling`). Applied under
    // `--workspace`, cargo turns the feature on for the members that define it
    // (`renzora_app` + `renzora_runtime`) and leaves the rest alone; feature
    // unification then propagates the Bevy features to the one shared `bevy_dylib`.
    if !features.is_empty() {
        args.push("--features".to_string());
        args.push(features.join(","));
    }
    println!("[xtask] cargo {}", args.join(" "));
    let ok = Command::new(cargo())
        .current_dir(repo)
        .args(&args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok && build_source_plugins(repo) && build_updater(repo)
}

/// Build the update sidecar (`tools/updater`).
///
/// Its own workspace, like `plugins/*`, so `--workspace` never sees it — and it
/// has to ship beside the editor or Help ▸ Check for Updates can find and
/// download an update with nothing to install it.
///
/// Never fatal: a missing sidecar costs the in-place update and nothing else.
/// The editor notices it isn't there and tells you to download the new version
/// by hand, which is a fine outcome for a dev build that will be rebuilt in a
/// minute anyway.
fn build_updater(repo: &Path) -> bool {
    let dir = repo.join("tools").join("updater");
    if !dir.join("Cargo.toml").exists() {
        return true;
    }
    println!("[xtask] cargo build --profile dist (updater)");
    let ok = Command::new(cargo())
        .current_dir(&dir)
        .args(["build", "--profile", "dist"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("[xtask] WARN: update sidecar failed to build — in-place updates disabled");
    }
    true
}

/// Build every `renzora_plugin` cdylib under `plugins/`.
///
/// They are excluded from the workspace on purpose — as members they would
/// inherit the engine's cargo feature unification and link Bevy, destroying the
/// zero-dependency property that is the whole point. The cost is that
/// `--workspace` never sees them, so without this step a stale copy from an
/// earlier ABI lingers in the staged `plugins/` and gets loaded. That failure is
/// nasty: a plugin built against an older ABI can pass the version handshake and
/// then be called with a signature it was not compiled for.
///
/// Each is its own tiny build — a quarter of a second once the shared
/// `plugins/target/` is warm, since only the plugin itself compiles.
fn build_source_plugins(repo: &Path) -> bool {
    let root = repo.join("plugins");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return true; // no plugins/ is fine
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.join("Cargo.toml").exists() {
            continue;
        }
        // Native plugins share this directory but not this build path. They link
        // Bevy, and `plugins/` sits outside the engine workspace — so cargo
        // would resolve them a FRESH Bevy from crates.io, with different
        // `TypeId`s, producing a plugin that builds cleanly and then corrupts
        // the World. They are compiled against the staged SDK instead, after it
        // exists. See `native_plugin.rs`.
        if native_plugin::is_native(&dir) {
            continue;
        }
        // Skip the ones nothing has touched. Cargo would reach the same
        // conclusion, but only after spawning a process, parsing a manifest and
        // fingerprinting a dependency graph — and this loop does that ~66 times
        // on a build where the answer is "no" every time. A `stat` per source
        // file gets there first.
        let stamp = plugin_stamp(repo, &dir);
        if let Some((stamp, path)) = &stamp {
            if std::fs::read_to_string(path).is_ok_and(|s| s.trim() == stamp) {
                continue;
            }
        }
        println!("[xtask] cargo build --profile dist ({})", file_name(&dir));
        let ok = Command::new(cargo())
            .current_dir(&dir)
            .args(["build", "--profile", "dist"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
        // Recorded only after a build that succeeded, so a failure is retried.
        if let Some((stamp, path)) = &stamp {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, stamp);
        }
    }
    true
}

/// What this plugin was last built from, and where that is recorded.
///
/// `None` when the inputs cannot be read, which skips the whole mechanism and
/// always builds — the safe direction. Skipping a build that was needed leaves a
/// plugin compiled against an older ABI: one that passes the version handshake
/// and is then called with a signature it was not compiled for. Running a build
/// that was not needed costs a second.
///
/// # Why a stamp rather than comparing against the built library
///
/// That was the obvious approach and it does not work. Cargo, asked to build a
/// plugin it considers current, prints `Finished` and **does not touch the
/// artifact** — so the library keeps an old mtime while a source file it did not
/// need to recompile (a comment in `renzora_plugin`, say) is newer. Compared
/// against the artifact, every plugin then reads as permanently stale and the
/// skip never fires. A stamp records the inputs as they were at the last
/// *successful* build, which is the question actually being asked.
///
/// The **path dependencies are part of the stamp**, and are the reason this is
/// not just the plugin's own `src/`. Every C-ABI plugin path-depends on
/// `renzora_plugin`, which *is* the ABI: edit that and every plugin must
/// rebuild even though nothing in its own directory moved.
fn plugin_stamp(repo: &Path, dir: &Path) -> Option<(String, PathBuf)> {
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let pkg = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("name = "))
        .and_then(|l| l.split('"').nth(1).map(|s| s.replace('-', "_")))
        .unwrap_or_else(|| file_name(dir).replace('-', "_"));

    // The library has to exist as well as the stamp matching: a wiped
    // `plugins/target/` leaves the stamp behind, and skipping then would stage
    // nothing and quietly ship a build with the plugin missing.
    let plat = platform();
    let artifact = repo
        .join("plugins")
        .join("target")
        .join(profile())
        .join(format!("{}{}.{}", plat.lib_prefix, pkg, plat.ext));
    if !artifact.is_file() {
        return None;
    }

    let mut rows = Vec::new();
    collect_rows(dir, dir, &mut rows)?;
    for line in manifest.lines() {
        let Some(rest) = line.split_once("path = ").map(|(_, r)| r) else {
            continue;
        };
        let rel = rest.split('"').nth(1)?;
        let dep = dir.join(rel);
        collect_rows(&dep, &dep, &mut rows)?;
    }
    rows.sort();

    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for r in &rows {
        for b in r.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(PRIME);
        }
    }

    let path = repo
        .join("plugins")
        .join("target")
        .join(".xtask-stamps")
        .join(format!("{pkg}.stamp"));
    Some((format!("{h:016x}"), path))
}

/// `relative path:len:mtime` for every file under `dir`, skipping build outputs.
///
/// `None` on any unreadable directory, which the caller turns into "build it" —
/// a stamp computed from a partial listing would match again next time and make
/// the skip permanent.
fn collect_rows(root: &Path, dir: &Path, out: &mut Vec<String>) -> Option<()> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            // Outputs, not inputs — and `target/` holds the very artifact this
            // is deciding about.
            if matches!(e.file_name().to_str(), Some("target") | Some("build")) {
                continue;
            }
            collect_rows(root, &p, out)?;
        } else {
            let meta = e.metadata().ok()?;
            let secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy();
            out.push(format!("{rel}:{}:{secs}", meta.len()));
        }
    }
    Some(())
}

/// Port of `build-all.sh`'s `copy_shared_libs`: arrange `target/dist/` into a
/// clean, runnable `dist/<platform>/`.
fn stage(repo: &Path, plat: &Platform) -> std::io::Result<PathBuf> {
    // The profile name IS the target-dir subdirectory, so staging follows
    // whatever `build` just produced rather than assuming `dist`.
    let src = repo.join("target").join(profile());
    let out = repo.join("dist").join(plat.dir);
    let plugins = out.join("plugins");
    std::fs::create_dir_all(&plugins)?;

    // Wipe prior artifacts so a removed plugin doesn't linger in dist/. Only the
    // exe + shared libs are swept; any other dist content (configs, assets a
    // packager dropped in) is left alone.
    clean_artifacts(&out, plat)?;
    clean_artifacts(&plugins, plat)?;

    // ── The two executables ──────────────────────────────────────────────────
    // `renzora`        the runtime / shipped game — the ONLY binary an export
    //                  copies. Contains no editor crate.
    // `renzora-editor` the editor. A separate executable rather than a loadable
    //                  bundle because play mode spawns `renzora` as a child
    //                  process, so the pair ships together either way.
    let bin_name = format!("renzora{}", plat.exe_suffix);
    let host_bin = out.join(&bin_name);
    link_or_copy(&src.join(&bin_name), &host_bin)?;
    #[cfg(unix)]
    make_executable(&host_bin)?;

    // The update sidecar, from its own target dir (own workspace).
    let updater_name = format!("renzora-update{}", plat.exe_suffix);
    let updater_src = repo
        .join("tools")
        .join("updater")
        .join("target")
        .join("dist")
        .join(&updater_name);
    if updater_src.exists() {
        let updater_bin = out.join(&updater_name);
        link_or_copy(&updater_src, &updater_bin)?;
        #[cfg(unix)]
        make_executable(&updater_bin)?;
    }

    // ── The editor image ─────────────────────────────────────────────────────
    // One binary, and the presence of this file is what makes it the editor.
    // `renzora` looks for it beside itself at startup; without it the same
    // executable is the shipped game, which is why an export simply does not
    // stage it rather than shipping a different binary.
    //
    // A `dylib`, not a second `.exe`. That was possible again the moment Bevy
    // became a shared image: the editor takes `&mut App` across the boundary,
    // which is sound only while both sides link one `bevy_dylib`, one
    // `renzora_dylib`, one `renzora_ember_dylib` and one `renzora_runtime_dylib`.
    let editor_name = format!("{}renzora_editor.{}", plat.lib_prefix, plat.ext);
    let editor_src = src.join(&editor_name);
    if editor_src.exists() {
        link_or_copy(&editor_src, &out.join(&editor_name))?;
    } else {
        eprintln!(
            "[xtask] WARN: {} missing — staged a runtime-only tree (build with `cargo dist`)",
            editor_name
        );
    }

    // ── OpenXR loader (VR) ───────────────────────────────────────────────────
    // The Khronos loader every OpenXR app must ship: `openxr::Entry::load()`
    // (the `--vr` boot / XR-capable editor probe) LoadLibrary's it from beside
    // the exe. Vendored under tools/openxr; Windows-only for now.
    if plat.ext == "dll" {
        let loader = repo.join("tools/openxr/openxr_loader.dll");
        if loader.exists() {
            link_or_copy(&loader, &out.join("openxr_loader.dll"))?;
        } else {
            eprintln!("[xtask] WARN: tools/openxr/openxr_loader.dll missing — VR won't initialize");
        }
    }

    // ── The shared libraries ─────────────────────────────────────────────────
    // AFTER the executables, because which ones to stage is read from their
    // import tables — there is nothing to read before they are copied. Getting
    // this order wrong stages no dylibs at all and produces a dist/ that cannot
    // start, which is exactly the failure this approach exists to prevent.
    stage_shared_libs(&src, &out, plat)?;

    // ── Distribution plugin cdylibs → plugins/ ───────────────────────────────
    let mut count = 0;
    for entry in std::fs::read_dir(&src)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = file_name(&path);
        if !name.ends_with(&format!(".{}", plat.ext)) || is_not_a_plugin(&name, plat) {
            continue;
        }
        link_or_copy(&path, &plugins.join(&name))?;
        count += 1;
    }

    // ── Source plugin cdylibs (plugins/) → plugins/ ──────────────────────────
    // Separate pass because they are separate cargo projects, so the workspace
    // sweep above never sees them.
    // One shared `plugins/target/` for all of them now (see
    // `plugins/.cargo/config.toml`), so this is a single directory read rather
    // than a walk over 61 per-plugin target dirs.
    let src_root = repo.join("plugins");
    let ex = src_root.join("target").join("dist");
    if let Ok(files) = std::fs::read_dir(&ex) {
        for f in files.flatten() {
            let path = f.path();
            let name = file_name(&path);
            if path.is_file() && name.ends_with(&format!(".{}", plat.ext)) {
                // Cargo never sweeps the shared `plugins/target/`, so deleting a
                // plugin's source leaves its cdylib sitting there and an
                // unfiltered copy would restage it on every build — the deleted
                // plugin appears to come back from the dead, and gets loaded
                // against an ABI it was not compiled for (`renzora_git` did
                // exactly this, spamming "panel op N is not one this build has").
                // Stage only what still has a source directory.
                let stem = name.trim_end_matches(&format!(".{}", plat.ext));
                // `lib` on Unix, empty on Windows — strip only when non-empty,
                // so `libgit.so` and `git.dll` both resolve to `plugins/git`.
                let stem = stem.strip_prefix(plat.lib_prefix).unwrap_or(stem);
                let manifest = src_root.join(stem).join("Cargo.toml");
                if !manifest.exists() {
                    println!("[xtask] skipping orphaned plugin artifact {name} (no plugins/{stem}/)");
                    continue;
                }
                // A directory existing is not enough: a plugin CONVERTED from
                // C-ABI to native keeps its name, so its stale cdylib in the
                // shared `plugins/target/` passes the check above and gets staged
                // beside the native plugin's directory. The two loaders are
                // supposed to be unable to collide — one scans loose files, the
                // other directories — and that holds for what the build
                // *produces*, but not for what a previous build left lying
                // around. `ai_chat` did exactly this: a loose `ai_chat.dll` and
                // an `ai_chat/` directory, same id, one disable switch between
                // them.
                //
                // So ask the manifest what the crate actually is now, and only
                // stage the loose artifact when it is still a C-ABI plugin.
                let is_cdylib = std::fs::read_to_string(&manifest)
                    .map(|s| s.contains("cdylib"))
                    .unwrap_or(false);
                if !is_cdylib {
                    println!(
                        "[xtask] skipping stale C-ABI artifact {name} \
                         (plugins/{stem}/ is a native plugin now)"
                    );
                    continue;
                }
                link_or_copy(&path, &plugins.join(&name))?;
                count += 1;
            }
        }
    }

    // Native macOS dylibs record their absolute build path as the install name;
    // rewrite to @rpath so the relocated dist/ folder actually resolves at run.
    #[cfg(target_os = "macos")]
    fixup_macos(&out);

    let root = std::fs::read_dir(&out)?.flatten().filter(|e| e.path().is_file()).count();
    println!("[xtask] staged {} ({root} root files, {count} plugins)", out.display());
    Ok(out)
}

/// Files that look like plugin cdylibs but must NOT be swept into `plugins/`:
/// the shared SDK dylibs (shipped beside the exe), Rust internals, the editor
/// bundle (loaded from beside the exe), and a few crates that emit a cdylib but
/// carry no plugin (`plugin_bevy_hash`) — the loader would reject them, and a
/// stale one from the cargo cache would ship as dead weight. Mirrors the skip
/// list in `build-all.sh`.
fn is_not_a_plugin(name: &str, plat: &Platform) -> bool {
    let p = plat.lib_prefix;
    let e = plat.ext;
    let is = |stem: &str| name == format!("{p}{stem}.{e}");
    name.contains("bevy_dylib")
        // The shared contract image, staged beside the exe. Sweeping it into
        // `plugins/` would make the C-ABI loader `dlopen` it looking for an
        // `INIT_SYMBOL` it does not export — harmless, but it would also ship a
        // second ~30 MB copy of a library already sitting one directory up.
        || name.contains("renzora_dylib")
        // The shared ember image, for the same reason. Its own entry rather than
        // a looser pattern: `contains("renzora_dylib")` does not match
        // `renzora_ember_dylib`, and widening it to `_dylib` would start
        // swallowing names a plugin author is entitled to choose.
        || name.contains("renzora_ember_dylib")
        || name.starts_with("std-")
        || name.starts_with("libstd-")
        // PROC-MACRO CRATES. These compile to a dylib for *rustc* to load, not for
        // us. Staging one is not merely useless: the C-ABI loader calls
        // `Library::new` on every dll in `plugins/`, and `dlopen`ing a proc-macro
        // dylib into a process that is not the compiler crashes the editor before
        // it reaches the splash. Any new `proc-macro = true` crate belongs here.
        || name.contains("renzora_macros")
        || name.contains("renzora_plugin_derive")
        || name.contains("avian_derive")
        || is("renzora")
        || is("renzora_editor")
        || is("renzora_editor_bundle") // pre-rename name, in case it lingers in cache
        || is("renzora_postprocess") // now an rlib shim; a stale dylib has no add!
        || is("renzora_preview") // wasm helper cdylib, not an engine plugin
}

/// Copy the shared libraries both executables import into `dist/<platform>/`.
///
/// Both are hard failures rather than warnings, because a missing one does not
/// produce a clean error at runtime: the OS loader refuses the executable before
/// `main` is reached, and the message names a hashed filename rather than
/// anything a reader can act on. Failing here, with a path, is the only place
/// that mistake is cheap to diagnose.
fn stage_shared_libs(src: &Path, out: &Path, plat: &Platform) -> std::io::Result<()> {
    // WHICH dylibs to stage is read from the executables' own import tables, not
    // by searching `deps/` for a likely-looking name.
    //
    // `deps/` routinely holds several `-C metadata` variants of one crate: a
    // `cargo test -p <package>` resolves features differently from a
    // `--workspace` build and leaves a second `bevy_dylib` behind. Taking "the
    // first one `read_dir` yields" then stages a dylib the binary does not
    // import, producing a dist/ that looks complete and dies before `main` with
    // a Windows dialog naming a hashed file nobody can place:
    //
    //     bevy_dylib-be9a16d285af241b.dll was not found
    //
    // An import table cannot drift from the binary carrying it, so it is the
    // only source that is right by construction. Same lesson as the SDK artifact
    // list, in a different place.
    //
    // Both executables are scanned: they are built from one `--workspace`
    // invocation and so agree today, but staging what each one actually asks for
    // costs nothing and cannot be wrong.
    let deps = src.join("deps");
    let mut wanted = std::collections::BTreeSet::new();
    // The executable and the editor image both import shared libraries, and the
    // image imports more of them than the exe does — it is the editor, so it
    // pulls the UI toolkit and every editor crate's dependencies. Asking both is
    // what keeps a staged tree complete now that the editor is a library rather
    // than a second executable that was scanned here.
    let host_binaries = [
        format!("renzora{}", plat.exe_suffix),
        format!("{}renzora_editor.{}", plat.lib_prefix, plat.ext),
    ];
    for name in &host_binaries {
        let path = out.join(name);
        if path.is_file() {
            wanted.extend(imported_libs(&path, plat));
        }
    }
    for name in &wanted {
        let from = deps.join(name);
        if from.is_file() {
            link_or_copy(&from, &out.join(name))?;
        }
    }

    // `std-<hash>` is named by the same import tables, but lives somewhere else:
    // the hash covers the toolchain, and nothing in `target/` holds a copy.
    // `--print target-libdir` resolves to the sysroot's per-target lib dir.
    let libdir = Command::new("rustc")
        .args(["--print", "target-libdir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
        .ok_or_else(|| {
            std::io::Error::other("`rustc --print target-libdir` failed — is rustc on PATH?")
        })?;
    for name in &wanted {
        let from = libdir.join(name);
        if from.is_file() {
            // The one source that is routinely on another volume — the rustup
            // sysroot. `link_or_copy` falls back for it without special-casing.
            link_or_copy(&from, &out.join(name))?;
        }
    }

    // Every name the binaries asked for has to have been found somewhere. A miss
    // is fatal rather than a warning, because the alternative is a dist/ that
    // starts no process and prints no error — the OS loader refuses it before
    // `main`, naming only a hashed filename.
    if let Some(missing) = wanted.iter().find(|n| !out.join(n).is_file()) {
        return Err(std::io::Error::other(format!(
            "{missing} is imported by the engine binaries but was found in neither \
             {} nor {}",
            deps.display(),
            libdir.display()
        )));
    }
    Ok(())
}

/// The Rust shared libraries an executable imports, by exact filename.
///
/// Read by scanning for the literal names rather than by walking the PE/ELF
/// import directory: the names are null-terminated strings in the binary, xtask
/// carries no dependencies to parse object files with, and a false positive
/// would have to be a string that is already the filename we would copy.
///
/// Restricted to the three stems that are ours to stage. System libraries
/// (`KERNEL32`, `VCRUNTIME140`) are the platform's business, and the `windows`
/// crate's import libraries are the SDK's.
fn imported_libs(exe: &Path, plat: &Platform) -> Vec<String> {
    let Ok(bytes) = std::fs::read(exe) else {
        return Vec::new();
    };
    let mut out = std::collections::BTreeSet::new();
    // Every shared image, plus the Rust std the binary imports. A stem missing
    // from this list is not staged, and the binary then fails to start with
    // Windows' "code execution cannot proceed" dialog naming it.
    for stem in ["bevy_dylib", "renzora_dylib", "renzora_ember_dylib", "std"] {
        let prefix = format!("{}{stem}", plat.lib_prefix);
        let suffix = format!(".{}", plat.ext);
        let pat = prefix.as_bytes();
        let mut i = 0;
        while let Some(at) = find_bytes(&bytes[i..], pat) {
            let start = i + at;
            // A filename is short; cap the scan so a stray match cannot run away.
            let end = (start + 96).min(bytes.len());
            if let Some(name) = std::str::from_utf8(&bytes[start..end])
                .ok()
                .and_then(|s| s.split('\0').next())
                .filter(|s| s.ends_with(&suffix))
                // `std` must not match `std_detect` or similar: after the stem
                // there is either the extension or `-<hash>`.
                .filter(|s| {
                    let rest = &s[prefix.len()..s.len() - suffix.len()];
                    rest.is_empty()
                        || (rest.starts_with('-')
                            && rest[1..].chars().all(|c| c.is_ascii_hexdigit()))
                })
            {
                out.insert(name.to_string());
            }
            i = start + pat.len();
        }
    }
    out.into_iter().collect()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Launch the staged editor. cwd = repo root so the editor resolves project
/// assets the same way a plain `cargo run` does; plugins resolve via the loader's
/// `<exe-dir>/plugins/` scan, independent of cwd.
///
/// `default_no_xr` makes the launch pass `RENZORA_NO_XR=1` (the profiling lane —
/// see the `profile` arm). `--xr` anywhere in the passthrough args cancels it, and
/// is consumed here rather than forwarded: the runtime doesn't know that flag, and
/// XR-capable boot is its default whenever a runtime is reachable, so simply *not*
/// setting the variable is what asks for it. An `RENZORA_NO_XR` already in the
/// environment wins either way — the runtime only tests for the variable's
/// presence, so an explicit one from the caller must not be second-guessed here.
fn launch(repo: &Path, out: &Path, plat: &Platform, default_no_xr: bool) -> ExitCode {
    let mut extra: Vec<String> = std::env::args().skip(2).collect();

    // One binary. `renzora` is the editor when `renzora_editor.<dll|so|dylib>`
    // sits beside it and the game when it does not, so there is no second
    // executable to choose between any more — this used to pick between
    // `renzora-editor` and `renzora`, from the years the editor was its own
    // exe.
    //
    // `--server`, `--host` and `--vr` still matter, but to the binary rather
    // than to this: it reads them in `main` to boot headless, as a listen
    // server, or into a headset, and each of those is never an editor session
    // even with the image present. So they are simply forwarded.
    let bin = out.join(format!("renzora{}", plat.exe_suffix));
    if !bin.exists() {
        eprintln!("[xtask] {} was not staged", bin.display());
        return ExitCode::FAILURE;
    }
    println!("[xtask] launching {}", bin.display());
    let want_xr = extra.iter().any(|a| a == "--xr");
    extra.retain(|a| a != "--xr");
    let mut cmd = Command::new(&bin);
    cmd.current_dir(repo).args(&extra);
    if default_no_xr && !want_xr && std::env::var_os("RENZORA_NO_XR").is_none() {
        println!(
            "[xtask] RENZORA_NO_XR=1 (flat, pipelined boot — an installed OpenXR \
             runtime would otherwise disable pipelined rendering and serialize the \
             render sub-app onto the main thread. Use `cargo renzora xr` to edit in \
             a headset.)"
        );
        cmd.env("RENZORA_NO_XR", "1");
    }
    match cmd.status() {
        Ok(s) => s.code().map(|c| ExitCode::from(c as u8)).unwrap_or(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("[xtask] failed to launch {}: {e}", bin.display());
            ExitCode::FAILURE
        }
    }
}

// ── small helpers ────────────────────────────────────────────────────────────

/// Honor cargo's chosen toolchain when xtask is itself invoked via cargo.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The value after `flag` in `argv`, as `--flag value`.
///
/// Only the separated form, not `--flag=value`: these are passed by
/// `docker/build-all.sh`, which writes them separated, and accepting a second
/// spelling would be untested surface.
fn flag_value(argv: &[String], flag: &str) -> Option<String> {
    argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned()
}

/// Which cargo profile the ENGINE is built with.
///
/// `dist` by default — the fast-link profile a contributor iterates with, and
/// the reason `[profile.dist]` exists at all.
///
/// `RENZORA_PROFILE=release` selects the size-optimised profile that actually
/// ships. `docker/build-all.sh` already reads the same variable, so the two
/// entry points now agree on one name rather than each holding its own idea of
/// what "the shipping build" is.
///
/// # Why this exists
///
/// The `windows-arm64` lane used to approximate `release` by setting
/// `CARGO_PROFILE_DIST_OPT_LEVEL` and `CARGO_PROFILE_DIST_LTO` on top of `dist`,
/// with a comment asking whoever edits `[profile.release]` to keep them in step.
/// Nothing enforced that, and the drift was invisible: a config that ships was
/// never built anywhere else, so `bevy_dylib` exceeding the PE export cap at
/// `opt-level = "s"` could only surface in CI, after a full cross-compile.
///
/// Naming the profile instead of reconstructing it removes that whole class —
/// and makes the shipping build reproducible locally with one variable.
///
/// The profile name doubles as cargo's target-dir subdirectory, so this also
/// moves where [`stage`] reads from. Plugin and updater builds deliberately do
/// NOT use it: each is its own workspace with its own tuned `[profile.dist]`,
/// exactly as `build-all.sh` documents.
pub(crate) fn profile() -> String {
    std::env::var("RENZORA_PROFILE").unwrap_or_else(|_| "dist".to_string())
}

/// Copy, naming the destination on failure.
///
/// `std::io::Error` from `fs::copy` carries no path, so a locked file surfaced as
/// a bare "The process cannot access the file because it is being used by another
/// process. (os error 32)" with nothing to act on. On Windows that error almost
/// always means the editor is still running and holding `renzora.exe`, which is
/// worth being told rather than guessing.
pub(crate) fn copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ()).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("{} -> {}: {e}", src.display(), dst.display()),
        )
    })
}

/// Hardlink `src` into `dst`, copying only if that fails.
///
/// Staging is a *view* of `target/dist/`, not a second set of the bytes.
/// `target/` and `dist/` are normally the same volume, so a link is instant and
/// consumes no additional disk — which is what lets the SDK stage ~1050 files on
/// every build, and what keeps `dist/<platform>/` from being a 389 MB duplicate
/// of files that already exist a directory away. (Measured: before this,
/// `renzora-editor.exe` at 155 MB, `bevy_dylib` at 124 MB and every plugin cdylib
/// were each stored twice.)
///
/// Deleting `target/` later does not break the staged tree — a hardlink is not a
/// reference to a file but one of its names, so the data survives while any name
/// remains.
///
/// # Why this is safe for the staged binaries and not merely for the SDK
///
/// A hardlink is only wrong if something rewrites the staged file **in place**,
/// which would reach back through the link and corrupt `target/`. Exactly one
/// step does that — UPX, in `docker/build-all.sh` — and it cannot collide here:
/// it runs only in the container, only on the three executables, and against a
/// tree that script staged itself with `cp`. Nothing on the native path modifies
/// a staged artifact after it lands; a rebuild replaces it, and
/// `clean_artifacts` + the `remove_file` below relink it from scratch.
///
/// The fallback matters for the cases where linking cannot work: `dist/` on a
/// different drive (or `std-<hash>.dll`, which comes from the rustup sysroot and
/// may be on another volume), or a filesystem without hardlinks. Those pay the
/// copy, which is what the code did before and is merely slow rather than wrong.
pub(crate) fn link_or_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    // A leftover from a previous stage would make `hard_link` fail with
    // AlreadyExists and silently fall through to a copy, so clear it first. This
    // is also what refreshes the link when a rebuild gave `src` a new inode.
    let _ = std::fs::remove_file(dst);
    match std::fs::hard_link(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => copy(src, dst),
    }
}

/// Remove only exe + shared-lib artifacts from a dir (keep everything else).
fn clean_artifacts(dir: &Path, plat: &Platform) -> std::io::Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let name = file_name(&entry.path());
        // On Unix the executables have no extension, so the suffix tests above
        // miss them and a renamed/removed binary would linger. Name them
        // explicitly.
        let is_unix_exe = plat.exe_suffix.is_empty()
            && matches!(
                name.as_str(),
                "renzora" | "renzora-editor" | "renzora-update"
            );
        if name.ends_with(&format!(".{}", plat.ext))
            || (!plat.exe_suffix.is_empty() && name.ends_with(plat.exe_suffix))
            || is_unix_exe
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}




#[cfg(unix)]
fn make_executable(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(p)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(p, perms)
}

/// Rewrite native-build absolute install names to `@rpath/<name>` so the staged
/// `dist/` resolves at runtime: the exe carries an `@loader_path` rpath (from
/// `.cargo/config.toml`), and plugins get `@loader_path/..` so their deps
/// resolve in the exe dir one level up. Best-effort — warns if Xcode's
/// `install_name_tool`/`codesign` aren't present.
#[cfg(target_os = "macos")]
fn fixup_macos(out: &Path) {
    let mut files: Vec<PathBuf> = Vec::new();
    let push_dylibs = |dir: &Path, files: &mut Vec<PathBuf>| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if file_name(&p).ends_with(".dylib") {
                    files.push(p);
                }
            }
        }
    };
    files.push(out.join("renzora"));
    push_dylibs(out, &mut files);
    let plugins = out.join("plugins");
    push_dylibs(&plugins, &mut files);

    for f in &files {
        if !f.exists() {
            continue;
        }
        let name = file_name(f);
        if name.ends_with(".dylib") {
            let _ = Command::new("install_name_tool")
                .args(["-id", &format!("@rpath/{name}")])
                .arg(f)
                .status();
        }
        // Rewrite any dependency recorded as an absolute build path under target/.
        if let Ok(o) = Command::new("otool").arg("-L").arg(f).output() {
            for line in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
                let dep = line.trim().split_whitespace().next().unwrap_or("");
                if dep.contains("/target/") && dep.starts_with('/') {
                    let base = dep.rsplit('/').next().unwrap_or(dep);
                    let _ = Command::new("install_name_tool")
                        .args(["-change", dep, &format!("@rpath/{base}")])
                        .arg(f)
                        .status();
                }
            }
        }
        if f.starts_with(&plugins) {
            let _ = Command::new("install_name_tool")
                .args(["-add_rpath", "@loader_path/.."])
                .arg(f)
                .status();
        }
        // install_name_tool invalidates the ad-hoc signature; arm64 macOS refuses
        // invalid signatures, so re-sign each touched file ad-hoc.
        let _ = Command::new("codesign").args(["-s", "-", "-f"]).arg(f).status();
    }
}
