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
//!   2. Stage `dist/<platform>/`: the two executables + the OpenXR loader beside
//!      each other, every plugin cdylib into `plugins/`.
//!   3. (`run` only) launch the staged editor.
//!
//! There are no shared `bevy_dylib` / `renzora` / Rust-`std` libraries to stage
//! any more — Bevy is statically linked into both executables, so each is
//! self-contained. That is also why the editor is a second *executable* rather
//! than the removable `renzora_editor.dll` it used to be: a cdylib linking a
//! static Bevy would carry its own copy of Bevy, and therefore its own `World`
//! type, so nothing could cross the boundary. Shipping a game is now "copy
//! `renzora`", not "copy everything except one dll".
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

mod coverage;
mod sync;
mod wasm;

/// Host-platform naming. Filled at compile time from `cfg!` because xtask is
/// built for — and run on — the very platform it stages for.
struct Platform {
    /// `dist/<dir>` — matches `build-all.sh`'s platform directory names.
    dir: &'static str,
    /// Shared-library extension, no dot (`dll` / `so` / `dylib`).
    ext: &'static str,
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
        "dist" => match build_and_stage(&repo, &plat, &[]) {
            Ok(out) => {
                println!("[xtask] staged {}", out.display());
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
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
        "wasm" => wasm::build_and_stage(&repo),
        other => {
            eprintln!(
                "[xtask] unknown command '{other}' \
                 (expected: run | xr | dist | wasm | plugin <name> | profile | \
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
        Ok(out) => Ok(out),
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
    let mut args: Vec<String> = [
        "build",
        "--profile",
        "dist",
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
    ok && build_source_plugins(repo)
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
    }
    true
}

/// Port of `build-all.sh`'s `copy_shared_libs`: arrange `target/dist/` into a
/// clean, runnable `dist/<platform>/`.
fn stage(repo: &Path, plat: &Platform) -> std::io::Result<PathBuf> {
    let src = repo.join("target").join("dist");
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
    //                  bundle because Bevy is statically linked now: a cdylib
    //                  linking static Bevy would carry its own copy of Bevy, and
    //                  therefore its own `World` type.
    //
    // Both are self-contained — no sibling `bevy_dylib`/`renzora`/std dylibs to
    // copy any more, which is why the shared-lib passes that used to live here
    // are gone. "Remove the editor" is now "ship the other file".
    let bin_name = format!("renzora{}", plat.exe_suffix);
    let host_bin = out.join(&bin_name);
    copy(&src.join(&bin_name), &host_bin)?;
    #[cfg(unix)]
    make_executable(&host_bin)?;

    let editor_name = format!("renzora-editor{}", plat.exe_suffix);
    let editor_src = src.join(&editor_name);
    if editor_src.exists() {
        let editor_bin = out.join(&editor_name);
        copy(&editor_src, &editor_bin)?;
        #[cfg(unix)]
        make_executable(&editor_bin)?;
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
            copy(&loader, &out.join("openxr_loader.dll"))?;
        } else {
            eprintln!("[xtask] WARN: tools/openxr/openxr_loader.dll missing — VR won't initialize");
        }
    }

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
        copy(&path, &plugins.join(&name))?;
        count += 1;
    }

    // ── Source plugin cdylibs (plugins/) → plugins/ ──────────────────────────
    // Separate pass because they are separate cargo projects, so the workspace
    // sweep above never sees them.
    // One shared `plugins/target/` for all of them now (see
    // `plugins/.cargo/config.toml`), so this is a single directory read rather
    // than a walk over 61 per-plugin target dirs.
    let ex = repo.join("plugins").join("target").join("dist");
    if let Ok(files) = std::fs::read_dir(&ex) {
        for f in files.flatten() {
            let path = f.path();
            let name = file_name(&path);
            if path.is_file() && name.ends_with(&format!(".{}", plat.ext)) {
                copy(&path, &plugins.join(&name))?;
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

    // The editor, unless asked for a runtime-only mode.
    //
    // `renzora-editor` and `renzora` became separate executables when Bevy went
    // static (see `stage`), and this launched `renzora` regardless — so
    // `cargo renzora` silently started the game instead of the editor. The
    // three flags below are the modes only the runtime binary understands: it
    // reads them in `main` to boot headless, as a listen server, or into a
    // headset. `renzora-editor` parses none of them, so forwarding one there
    // would start an ordinary editor session and quietly ignore the request.
    const RUNTIME_ONLY: [&str; 3] = ["--server", "--host", "--vr"];
    let runtime_mode = extra.iter().any(|a| RUNTIME_ONLY.contains(&a.as_str()));
    let stem = if runtime_mode { "renzora" } else { "renzora-editor" };

    let bin = out.join(format!("{stem}{}", plat.exe_suffix));
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

/// Copy, naming the destination on failure.
///
/// `std::io::Error` from `fs::copy` carries no path, so a locked file surfaced as
/// a bare "The process cannot access the file because it is being used by another
/// process. (os error 32)" with nothing to act on. On Windows that error almost
/// always means the editor is still running and holding `renzora.exe`, which is
/// worth being told rather than guessing.
fn copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ()).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("{} -> {}: {e}", src.display(), dst.display()),
        )
    })
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
        let is_unix_exe =
            plat.exe_suffix.is_empty() && matches!(name.as_str(), "renzora" | "renzora-editor");
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
