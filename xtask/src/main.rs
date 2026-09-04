//! `cargo renzora` — native build + stage + run, the local mirror of Docker.
//!
//! Docker gives a contributor two things: a pinned toolchain (so everyone
//! compiles with the same rustc) and a build that arranges the output into a
//! runnable `dist/` layout. The toolchain pin is handled by `rust-toolchain.toml`
//! (rustup auto-selects 1.95.0). This binary handles the second half WITHOUT a
//! container, for the host platform only:
//!
//!   1. `cargo build --profile dist --workspace` — compile BOTH executables
//!      (`renzora`, the runtime/shipped game, and `renzora-editor`). One
//!      invocation: the ~700 shared crates compile once and are linked into
//!      both binaries, plugin crates among them.
//!   2. Stage `dist/<platform>/`: the two executables + the shared libraries +
//!      the OpenXR loader beside each other.
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
//! Why a staging step at all: `cargo run` leaves the executables in
//! `target/dist/` with the shared libraries they import scattered among ~700
//! other build artefacts, and nothing arranges the layout the OS loader and the
//! editor both expect — the exe pair, the shared images beside them, `sdk/` and
//! `plugins/` alongside. Step 2 is the one thing `cargo run` can't do, and the
//! only reason `cargo renzora` exists.
//!
//! `plugins/` is NOT staged into. It belongs to the editor: the marketplace
//! installs there, and a plugin is built where it is installed. Nothing in this
//! repository writes to it, which is also why nothing in this repository sweeps
//! it.
//!
//! Cross-platform builds stay Docker-only (`renzora build` / `build-all.sh`):
//! this tool only ever produces artifacts for the machine it runs on.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod bundle;
mod coverage;
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

    // `sync` and `remove` REPAIR a workspace (they prune the dangling generated
    // dependency that stops it loading), so they have to keep working on a
    // machine that cannot yet link. Every other command compiles something.
    if !matches!(cmd.as_str(), "sync" | "remove") {
        if let Err(code) = preflight_host_deps() {
            return code;
        }
    }

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
                 (expected: run | xr | dist | sdk | wasm [--no-opt] | profile | \
                 coverage [--check|--bless] | sync [--check] | remove <crate-name>)"
            );
            ExitCode::from(2)
        }
    }
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
            // Inside `sdk/`, and after it, because the two ship as one archive:
            // `sdk.tar.zst` is unpacked by REPLACING `sdk/`, so a plugin API
            // staged beside it would be deleted by the first update. It is not
            // part of the SDK in any other sense — see `standalone.rs`.
            if let Err(e) = stage_plugin_api(repo, &out) {
                eprintln!("[xtask] staging the plugin API failed: {e}");
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
    ok && build_updater(repo)
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

/// Stage the two crates a standalone plugin compiles against, as SOURCE.
///
/// This is the standalone counterpart of the SDK, and the comparison is the
/// point: the SDK is ~444 MB of crate metadata pinned to one exact rustc,
/// because a native plugin links the engine's own Bevy. A standalone plugin
/// links nothing, so all it needs is `renzora_plugin` and the derive crate
/// behind it — about 2 MB of `.rs`, compiled by whatever rustc the user has.
///
/// It lands in `sdk/plugin-api/` because the SDK is unpacked by replacing `sdk/`
/// wholesale, so that is the only place beside the editor an update cannot
/// delete. What a plugin's manifest says — `path = "../../crates/renzora_plugin"`,
/// the source-checkout location — does not resolve there, and is repointed by
/// `renzora_native_plugin::standalone::repoint_contract` before the build. That
/// is the more robust arrangement anyway: a plugin authored outside this
/// repository can declare anything, and no layout makes every such path resolve.
///
/// A prebuilt `.rlib` would be smaller and is the wrong answer: rlib metadata is
/// pinned to one rustc, so shipping one would reintroduce `error[E0514]` — the
/// SDK's toolchain lock — in the one mechanism that exists to avoid it.
fn stage_plugin_api(repo: &Path, out: &Path) -> std::io::Result<()> {
    for name in ["renzora_plugin", "renzora_plugin_derive"] {
        let src = repo.join("crates").join(name);
        let dst = out.join("sdk").join("plugin-api").join(name);
        std::fs::create_dir_all(&dst)?;
        copy_tree(&src.join("src"), &dst.join("src"))?;
        let manifest = std::fs::read_to_string(src.join("Cargo.toml"))?;
        std::fs::write(dst.join("Cargo.toml"), deworkspace(&manifest))?;
    }
    Ok(())
}

/// Rewrite a workspace member's manifest so it stands alone.
///
/// Cargo resolves `workspace = true` when it PARSES a manifest, before it looks
/// at features — so `bevy = { workspace = true, optional = true }` fails outside
/// the workspace even though nothing a plugin builds ever enables it. Two
/// inheritances to undo, and both are fatal rather than cosmetic:
///
///   * `bevy` takes the version the workspace pins.
///   * `[lints] workspace = true` is dropped; a lint table is not part of what a
///     plugin compiles against.
fn deworkspace(manifest: &str) -> String {
    let mut out = String::with_capacity(manifest.len());
    let mut in_lints = false;
    for line in manifest.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_lints = trimmed.starts_with("[lints]");
            if in_lints {
                continue;
            }
        }
        if in_lints {
            continue;
        }
        if trimmed.starts_with("bevy = { workspace = true") {
            out.push_str(&line.replace("workspace = true", "version = \"0.19\""));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Copy a source tree, recursively.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let p = e.path();
        let dst = to.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &dst)?;
        } else if p.is_file() {
            // Byte-compared rather than copied unconditionally: `fs::copy` does
            // not preserve mtime, and the editor decides whether a plugin needs
            // rebuilding by comparing source mtimes against the artefact. An
            // unconditional copy would make every plugin look edited after every
            // `cargo renzora`, and rebuild the lot on the next launch.
            if std::fs::read(&p).ok() != std::fs::read(&dst).ok() {
                copy(&p, &dst)?;
            }
        }
    }
    Ok(())
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

    // Wipe prior artifacts so a removed binary or shared library doesn't linger
    // in dist/. Only the exe + shared libs are swept; any other dist content
    // (configs, assets a packager dropped in) is left alone.
    //
    // NOT `plugins/`. Nothing staged there is ours any more — the directory now
    // holds what the marketplace installed and what a user dropped in, and a
    // sweep of every `.so` in it would delete exactly that. It was swept when
    // xtask put the files there; it must not be now that it doesn't.
    clean_artifacts(&out, plat)?;

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

    // ── Engine resources ─────────────────────────────────────────────────────
    // Under `resources/` rather than at the root, where a loose `icon.png` sits
    // beside `renzora` and the shared libraries and says nothing about which of
    // them it belongs to — the editor's icon, the exported game's default, or
    // something safe to delete. The install's other directories each answer that
    // question by name (`plugins/`, `sdk/`, `tools/`) and this one does too.
    //
    // Staged at all because a DOWNLOADED editor has no repository to read from,
    // and the exporter needs a real icon to fall back on: `appimagetool` refuses
    // an AppDir whose `.desktop` names a file that is not there, and a `.app`
    // with no icon shows the generic application icon, which reads as a broken
    // bundle. The author's own icon still wins; this is the default for a
    // project that set none.
    let icon = repo.join("icon.png");
    if icon.is_file() {
        let resources = out.join("resources");
        std::fs::create_dir_all(&resources)?;
        link_or_copy(&icon, &resources.join("icon.png"))?;
    }

    // No plugin pass. In-workspace plugins are rlibs linked straight into the
    // binaries staged above, and third-party ones — C-ABI or native — arrive
    // through the marketplace into `plugins/`, which the editor owns. A build
    // tool that also wrote there could only fight it.

    // Native macOS dylibs record their absolute build path as the install name;
    // rewrite to @rpath so the relocated dist/ folder actually resolves at run.
    #[cfg(target_os = "macos")]
    fixup_macos(&out);

    let root = std::fs::read_dir(&out)?.flatten().filter(|e| e.path().is_file()).count();
    println!("[xtask] staged {} ({root} root files)", out.display());
    Ok(out)
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
/// The canonical Linux host build dependencies, per package manager.
///
/// The Debian row is the SECOND SOURCE OF TRUTH — keep it in lockstep with
/// `docker/base/Dockerfile` and the native Linux lane in
/// `.github/workflows/build-engine.yml`. All three describe the same set from a
/// different angle: container, CI runner, contributor's machine.
///
/// Every row holds the same libraries in the same order; only the names differ,
/// and they differ a lot — ALSA's headers are `libasound2-dev` on Debian,
/// `alsa-lib-devel` on Fedora and plain `alsa-lib` on Arch, where the headers
/// ship in the main package rather than a split `-dev` one. That is the whole
/// reason this is a table and not one list with the package manager swapped.
///
/// Only the Debian row is exercised by this repo's CI and container. The others
/// are best-effort: a wrong name makes the install fail and fall back to the
/// printed instructions, which is why nothing here is destructive if it is off.
#[cfg(target_os = "linux")]
const DEBIAN_PACKAGES: &[&str] = &[
    "pkg-config",
    "libx11-dev",
    "libxi-dev",
    "libxcursor-dev",
    "libxrandr-dev",
    "libxinerama-dev",
    "libwayland-dev",
    "libxkbcommon-dev",
    "libasound2-dev",
    "libudev-dev",
    "libvulkan-dev",
    "libssl-dev",
    // Not in `docker/base/Dockerfile` or the CI lane: the `rust:*-bookworm`
    // image and the GitHub runner image both already ship fontconfig, so a
    // missing one is invisible to Docker AND to CI and shows up only on a real
    // host, as a `yeslogic-fontconfig-sys` build-script panic minutes in.
    "libfontconfig1-dev",
    "clang",
    "mold",
];

#[cfg(target_os = "linux")]
const FEDORA_PACKAGES: &[&str] = &[
    "pkgconf-pkg-config",
    "libX11-devel",
    "libXi-devel",
    "libXcursor-devel",
    "libXrandr-devel",
    "libXinerama-devel",
    "wayland-devel",
    "libxkbcommon-devel",
    "alsa-lib-devel",
    "systemd-devel",
    "vulkan-loader-devel",
    "openssl-devel",
    "fontconfig-devel",
    "clang",
    "mold",
];

#[cfg(target_os = "linux")]
const ARCH_PACKAGES: &[&str] = &[
    "pkgconf",
    "libx11",
    "libxi",
    "libxcursor",
    "libxrandr",
    "libxinerama",
    "wayland",
    "libxkbcommon",
    "alsa-lib",
    "systemd-libs",
    "vulkan-icd-loader",
    "openssl",
    "fontconfig",
    "clang",
    "mold",
];

#[cfg(target_os = "linux")]
const SUSE_PACKAGES: &[&str] = &[
    "pkg-config",
    "libX11-devel",
    "libXi-devel",
    "libXcursor-devel",
    "libXrandr-devel",
    "libXinerama-devel",
    "wayland-devel",
    "libxkbcommon-devel",
    "alsa-devel",
    "systemd-devel",
    "vulkan-devel",
    "libopenssl-devel",
    "fontconfig-devel",
    "clang",
    "mold",
];

/// nixpkgs attribute names, for the `nix-shell` line printed on NixOS.
#[cfg(target_os = "linux")]
const NIX_PACKAGES: &[&str] = &[
    "pkg-config",
    "xorg.libX11",
    "xorg.libXi",
    "xorg.libXcursor",
    "xorg.libXrandr",
    "xorg.libXinerama",
    "wayland",
    "libxkbcommon",
    "alsa-lib",
    "systemd",
    "vulkan-loader",
    "openssl",
    "fontconfig",
    "clang",
    "mold",
];

/// Which package manager this machine actually has.
///
/// NixOS is detected FIRST and deliberately never installs. It is declarative:
/// the system profile is built from a committed configuration, so there is no
/// imperative "install this" to call — `nix-env -i` would touch only a user
/// profile and still not put the headers where a build can see them. The
/// correct answer there is a shell that provides them, so that is what gets
/// printed. `.cargo/config.toml` already accounts for NixOS hosts in the
/// `-fuse-ld=mold` comment, so they are an expected audience here.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq)]
enum Distro {
    Nixos,
    Debian,
    Fedora,
    Arch,
    Suse,
    Unknown,
}

#[cfg(target_os = "linux")]
fn detect_distro() -> Distro {
    if Path::new("/etc/NIXOS").exists() || have_bin("nixos-version") {
        return Distro::Nixos;
    }
    for (bin, distro) in [
        ("apt-get", Distro::Debian),
        ("dnf", Distro::Fedora),
        ("pacman", Distro::Arch),
        ("zypper", Distro::Suse),
    ] {
        if have_bin(bin) {
            return distro;
        }
    }
    Distro::Unknown
}

#[cfg(target_os = "linux")]
impl Distro {
    fn packages(self) -> &'static [&'static str] {
        match self {
            Distro::Debian => DEBIAN_PACKAGES,
            Distro::Fedora => FEDORA_PACKAGES,
            Distro::Arch => ARCH_PACKAGES,
            Distro::Suse => SUSE_PACKAGES,
            Distro::Nixos => NIX_PACKAGES,
            Distro::Unknown => DEBIAN_PACKAGES,
        }
    }

    /// The command a human should run, as a single copy-pasteable line.
    fn manual_command(self) -> String {
        let p = self.packages().join(" ");
        match self {
            Distro::Debian => format!("sudo apt install {p}"),
            Distro::Fedora => format!("sudo dnf install {p}"),
            Distro::Arch => format!("sudo pacman -S --needed {p}"),
            Distro::Suse => format!("sudo zypper install {p}"),
            Distro::Nixos => format!("nix-shell -p {p}"),
            Distro::Unknown => format!("<your package manager>: {p}"),
        }
    }

    /// The argv this tool runs to install unattended, or `None` where
    /// installing on the developer's behalf is not a thing that exists.
    fn install_argv(self) -> Option<Vec<String>> {
        let own = |v: &[&str]| -> Vec<String> {
            v.iter()
                .map(|s| (*s).to_string())
                .chain(self.packages().iter().map(|s| (*s).to_string()))
                .collect()
        };
        match self {
            Distro::Debian => Some(own(&["apt-get", "install", "-y", "--no-install-recommends"])),
            Distro::Fedora => Some(own(&["dnf", "install", "-y"])),
            Distro::Arch => Some(own(&["pacman", "-S", "--needed", "--noconfirm"])),
            Distro::Suse => Some(own(&["zypper", "--non-interactive", "install"])),
            // Declarative: nothing to run. See the enum docs.
            Distro::Nixos | Distro::Unknown => None,
        }
    }

    fn why_no_install(self) -> &'static str {
        match self {
            Distro::Nixos => {
                "NixOS is declarative, so nothing can install these imperatively \
                 — enter a shell that provides them"
            }
            _ => "no supported package manager was found",
        }
    }
}

/// `pkg-config` names for the dev libraries above.
///
/// The `-dev` packages install headers and `.pc` files, not binaries, so a PATH
/// lookup cannot see them — asking `pkg-config` is the only honest check.
///
/// Unlike the package tables, this list is the SAME on every distro: a `.pc`
/// file's name is set by the library's own build, not by whoever packaged it.
/// So detection is universal and only the remedy is distro-specific — which is
/// why a NixOS or Fedora host still gets an accurate list of what it is missing
/// even where this tool cannot install anything.
#[cfg(target_os = "linux")]
const LINUX_PROBES: &[&str] = &[
    "x11",
    "xi",
    "xcursor",
    "xrandr",
    "xinerama",
    "wayland-client",
    "xkbcommon",
    "alsa",
    "libudev",
    "vulkan",
    "openssl",
    "fontconfig",
];

#[cfg(target_os = "linux")]
fn have_bin(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Everything from [`LINUX_PACKAGES`] this machine cannot currently satisfy.
#[cfg(target_os = "linux")]
fn linux_missing() -> Vec<String> {
    let mut missing: Vec<String> = ["clang", "mold", "pkg-config"]
        .into_iter()
        .filter(|t| !have_bin(t))
        .map(String::from)
        .collect();

    // Without pkg-config the probes below cannot run at all; it is in the
    // install set, so the next pass sees the real answer.
    if have_bin("pkg-config") {
        for probe in LINUX_PROBES {
            let ok = Command::new("pkg-config")
                .args(["--exists", probe])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                missing.push((*probe).to_string());
            }
        }
    }
    missing
}

/// Run a package manager, as root directly or via `sudo`, terminal attached.
///
/// stdio is INHERITED rather than captured: `sudo` prompts for a password on
/// the controlling terminal, and a captured prompt is an invisible hang.
#[cfg(target_os = "linux")]
fn run_privileged(root: bool, argv: &[String]) -> bool {
    let (head, tail) = argv.split_first().expect("argv is never empty");
    let mut cmd = if root {
        Command::new(head)
    } else {
        let mut c = Command::new("sudo");
        c.arg(head);
        c
    };
    cmd.args(tail)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Print the RIGHT instructions for this machine and give up.
///
/// The command printed here is the detected distro's, never a hardcoded
/// `apt` line — telling a Fedora or NixOS user to run `sudo apt install` is
/// worse than saying nothing, because it looks authoritative and cannot work.
#[cfg(target_os = "linux")]
fn linux_manual(distro: Distro, missing: &[String], why: &str) -> Result<(), ExitCode> {
    eprintln!("[xtask] {why}, so set them up yourself:");
    eprintln!("[xtask]   {}", distro.manual_command());
    eprintln!("[xtask] missing: {}", missing.join(", "));
    if distro == Distro::Nixos {
        eprintln!(
            "[xtask] (or add them to your flake/configuration.nix devShell — \
             this repo ships no nix expression yet)"
        );
    }
    Err(ExitCode::FAILURE)
}

/// Install the Linux host build dependencies before anything tries to compile.
///
/// `[target.x86_64-unknown-linux-gnu]` in `.cargo/config.toml` links with
/// `clang` driving `mold`, and Bevy needs the X11/Wayland/ALSA/udev/Vulkan dev
/// libraries on top. None of it ships in a stock desktop install, so the first
/// thing a contributor used to see was rustc's bare
///
///     error: linker `clang` not found
///
/// — one of fourteen missing pieces, named after several hundred crates had
/// already compiled, with no package attached. Fixing that one walked into
/// `invalid linker name in argument '-fuse-ld=mold'`, and fixing THAT walked
/// into a `pkg-config` failure from `alsa-sys`. This ends the whole staircase in
/// one step, before the first crate builds.
///
/// This can only run because the `renzora` alias pins xtask's own bootstrap to
/// `cc` + `ld.bfd`; see the comment on that alias for the deadlock it breaks —
/// the tool that installs the linker must not itself need it.
///
/// ## When it declines to install
///
/// Automatic means automatic on a developer's machine, not everywhere:
///
///   * `CI` set, or stdin is not a terminal — `sudo` cannot prompt for a
///     password with nowhere to prompt, so it would hang rather than fail. Both
///     CI and the container install these themselves anyway.
///   * `RENZORA_NO_AUTO_INSTALL` set — the escape hatch, for anyone who wants
///     their package manager left alone.
///   * No `apt-get`. Package names genuinely differ across distros
///     (`libasound2-dev` is `alsa-lib-devel` on Fedora, `alsa-lib` on Arch), and
///     guessing them ships a command that fails halfway. Debian/Ubuntu/Mint is
///     what the container and CI both use, so it is the one set kept honest.
///
/// Each of those prints the exact `apt install` line and stops.
#[cfg(target_os = "linux")]
fn preflight_host_deps() -> Result<(), ExitCode> {
    let missing = linux_missing();
    if missing.is_empty() {
        return Ok(());
    }

    let distro = detect_distro();
    eprintln!("[xtask] missing Linux host build dependencies: {}", missing.join(", "));

    if std::env::var_os("RENZORA_NO_AUTO_INSTALL").is_some() {
        return linux_manual(distro, &missing, "RENZORA_NO_AUTO_INSTALL is set");
    }
    if std::env::var_os("CI").is_some() {
        return linux_manual(distro, &missing, "this is CI");
    }
    let Some(argv) = distro.install_argv() else {
        return linux_manual(distro, &missing, distro.why_no_install());
    };
    {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            return linux_manual(distro, &missing, "there is no terminal for sudo to prompt on");
        }
    }

    let root = Command::new("id")
        .arg("-u")
        .output()
        .map(|o| o.stdout.starts_with(b"0"))
        .unwrap_or(false);

    eprintln!(
        "[xtask] installing them now{}",
        if root { "" } else { " (sudo may ask for your password)" }
    );

    if !run_privileged(root, &argv) {
        // Overwhelmingly a stale index (`404  Not Found` on a moved pool path)
        // rather than a genuinely absent package, and a refresh is slow enough
        // to be worth skipping on the common path.
        if let Some(refresh) = match distro {
            Distro::Debian => Some(vec!["apt-get".to_string(), "update".to_string()]),
            Distro::Arch => Some(vec!["pacman".to_string(), "-Sy".to_string()]),
            Distro::Suse => Some(vec!["zypper".to_string(), "refresh".to_string()]),
            // dnf refreshes its metadata on demand.
            Distro::Fedora | Distro::Nixos | Distro::Unknown => None,
        } {
            eprintln!("[xtask] install failed; refreshing the package index and retrying once");
            run_privileged(root, &refresh);
            if run_privileged(root, &argv) {
                return finish(distro);
            }
        }
        return linux_manual(distro, &missing, "the automatic install failed");
    }

    finish(distro)
}

/// Re-probe after an install: the package manager reporting success is not the
/// same claim as the build now being able to find the libraries.
#[cfg(target_os = "linux")]
fn finish(distro: Distro) -> Result<(), ExitCode> {
    let still = linux_missing();
    if !still.is_empty() {
        return linux_manual(distro, &still, "the install ran but left some unsatisfied");
    }
    eprintln!("[xtask] host dependencies installed; continuing");
    Ok(())
}

/// Windows links with `rust-lld` from the rustc sysroot and macOS with the
/// system `cc`, and both get their platform SDKs from the OS toolchain rather
/// than from packages — so there is nothing to install.
#[cfg(not(target_os = "linux"))]
fn preflight_host_deps() -> Result<(), ExitCode> {
    Ok(())
}

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

#[cfg(test)]
mod plugin_api_tests {
    use super::deworkspace;

    /// Cargo resolves `workspace = true` when it PARSES a manifest, before it
    /// looks at features — so an unresolved inheritance is a hard error even for
    /// a dependency nothing enables. Both of these are fatal in the staged copy,
    /// and their failure mode is every standalone plugin refusing to build with
    /// an error about the API crate rather than about itself.
    #[test]
    fn workspace_inheritance_is_resolved_away() {
        let out = deworkspace(
            "[package]\nname = \"renzora_plugin\"\n\n             [dependencies]\n             bevy = { workspace = true, optional = true }\n             libm = { version = \"0.2\", optional = true }\n\n             [lints]\nworkspace = true\n",
        );
        assert!(!out.contains("workspace = true"), "{out}");
        assert!(out.contains("bevy = { version = \"0.19\", optional = true }"), "{out}");
        assert!(!out.contains("[lints]"), "{out}");
        // Everything else survives untouched — this is a repair, not a rewrite.
        assert!(out.contains("libm = { version = \"0.2\", optional = true }"), "{out}");
        assert!(out.contains("name = \"renzora_plugin\""), "{out}");
    }

    /// `[lints]` is dropped by skipping to the next table header, so whatever
    /// follows it must survive.
    #[test]
    fn a_table_after_lints_is_kept() {
        let out = deworkspace("[lints]\nworkspace = true\n\n[features]\ndefault = []\n");
        assert!(out.contains("[features]"), "{out}");
        assert!(out.contains("default = []"), "{out}");
    }
}
