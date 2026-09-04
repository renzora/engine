//! Lean static build backend — compiles the game's `renzora` binary from source
//! into a single stripped executable (static Bevy + static std, no sibling
//! dylibs), instead of copying the dynamically-linked dev runtime.
//!
//! A project is a separate asset folder, NOT a Rust workspace — so this compiles
//! the **engine source checkout** the editor was built from (located by walking
//! up from the editor's `dist/<platform>/` dir, see [`find_engine_source`]) and
//! the project's assets ride along in the rpak the caller appends. It builds via
//! `--no-default-features --features runtime` so the `dynamic_linking` feature is
//! dropped (see root `Cargo.toml`), under `[profile.dist-lean]`.
//!
//! The one subtlety is `prefer-dynamic`: `.cargo/config.toml` pins it per target
//! to make the *dev* build share one `bevy_dylib`. Cargo takes RUSTFLAGS from a
//! single highest-priority source with no merging, so setting
//! `CARGO_ENCODED_RUSTFLAGS` on the child process makes cargo **ignore** the
//! config rustflags for this one invocation — dropping `prefer-dynamic` without
//! editing any file. The separate `linker` config key is *not* rustflags and
//! survives (so Windows keeps `rust-lld`); on Linux we override it to the
//! near-universal `cc` because a freshly provisioned toolchain may lack the
//! repo's pinned `clang`/`mold`.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::templates::Platform;
use crate::toolchain::Toolchain;

/// Per-host RUSTFLAGS for the lean build, encoded for `CARGO_ENCODED_RUSTFLAGS`
/// (`\x1f`-separated args). An *empty* string still counts as "set", so cargo
/// drops the config's `prefer-dynamic` (and the Linux `mold` link-arg) — exactly
/// what we want for a static binary.
fn encoded_rustflags(platform: Platform) -> String {
    match platform {
        // Static-link the MSVC CRT too: with no dylib/TypeId boundary in a lean
        // binary, the reason it's disabled globally (crt-static perturbs crate
        // disambiguators across the dylib ABI) no longer applies — and it drops
        // the VCRUNTIME140.dll runtime dependency.
        Platform::WindowsX64 => ["-C", "target-feature=+crt-static"].join("\u{1f}"),
        // Drop prefer-dynamic (+ mold/rpath) by overriding with no flags.
        _ => String::new(),
    }
}

/// Drop `prefer-dynamic` from the export copy's cargo config, for a build that
/// runs in a container.
///
/// The host path does this with `CARGO_ENCODED_RUSTFLAGS`, which cargo treats as
/// a single highest-priority source and does not merge with any config file.
/// That is exactly what makes it work natively — and exactly why it cannot be
/// used in a container. The toolchain images supply the cross-linker's library
/// search paths *as rustflags*:
///
/// ```toml
/// [target.x86_64-pc-windows-msvc]
/// linker = "lld-link"
/// rustflags = ["-Lnative=/xwin/crt/lib/x86_64", …]
/// ```
///
/// Setting the env var would replace those, and the link would fail on missing
/// system libraries with nothing pointing at the cause. Editing the copy's own
/// config instead leaves cargo free to merge the two files' `rustflags` arrays
/// the way it normally does, so the image's paths survive.
///
/// Only the copy is touched — the dev tree is never edited (see
/// [`sync_export_workspace`]).
fn patch_cross_cargo_config(
    ws: &Path,
    platform: Platform,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let path = ws.join(".cargo").join("config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        // No config in the copy is fine: the image's own config then applies
        // unopposed, which is already what we want.
        return Ok(());
    };

    // Line-wise rather than a TOML round-trip: `prefer-dynamic` appears inside
    // `rustflags` arrays whose other entries must survive verbatim, and dropping
    // one array element is the whole edit.
    let mut out = String::with_capacity(text.len());
    let mut removed = 0usize;
    for line in text.lines() {
        if line.contains("prefer-dynamic") {
            removed += 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    // A lean Windows binary also static-links the MSVC CRT, which the host path
    // gets from `encoded_rustflags`. Appended as its own target section so it
    // merges with the image's rustflags rather than replacing them.
    if matches!(platform, Platform::WindowsX64) {
        out.push_str(
            "\n[target.x86_64-pc-windows-msvc]\nrustflags = [\"-C\", \"target-feature=+crt-static\"]\n",
        );
    }

    std::fs::write(&path, out)
        .map_err(|e| format!("Could not patch {} for a container build: {e}", path.display()))?;
    progress(format!("Prepared cargo config for a container build ({removed} dynamic-link flags removed)"));
    Ok(())
}

/// The lean binary's filename under `target/dist-lean/`.
fn bin_filename(platform: Platform) -> &'static str {
    match platform {
        // Both Windows arches now that a cross build can produce arm64 — the
        // extension follows the target, not the machine doing the compiling.
        Platform::WindowsX64 | Platform::WindowsArm64 => "renzora.exe",
        // A `[[bin]]` compiled for wasm32 is still a bin — rustc just emits a
        // module instead of an executable. This is the raw one, before
        // `wasm-bindgen` splits it into glue plus `<bundle>_bg.wasm`.
        Platform::WebWasm32 => "renzora.wasm",
        _ => "renzora",
    }
}

/// The cargo feature that selects a platform's runtime shape.
///
/// The web needs `wasm` rather than `runtime` — it turns on `bevy/webgpu` and
/// switches `src/main.rs` to the `#[wasm_bindgen]` `set_rpak`/`start` pair
/// instead of a native `main`. `wasm` itself enables `runtime`, so this is a
/// replacement rather than an addition.
fn runtime_feature(platform: Platform) -> &'static str {
    match platform {
        Platform::WebWasm32 => "wasm",
        _ => "runtime",
    }
}

/// Locate the engine source checkout to compile, by walking up from `start`
/// (the editor's runtime dir, e.g. `<engine>/dist/windows-x64/`).
///
/// A lean build recompiles the engine itself — projects are separate asset
/// folders with no Rust source — so we need the workspace root. We identify it by
/// its signature: a `Cargo.toml` plus a `crates/` dir plus `src/main.rs` (so a
/// sub-crate's `Cargo.toml` can't be mistaken for the root). Returns `None` for a
/// canonical editor release with no source beside it — lean builds there will
/// need the engine source fetched first (future work).
/// The directory the running editor lives in — where a lean build looks for the
/// engine source.
///
/// Deliberately NOT the target platform's runtime directory. That is the
/// *template* dir, and for a cross-platform export it is the per-user download
/// store (`~/.renzora/templates/<version>/<platform>/`), which has no engine
/// source anywhere above it:
///
/// ```text
/// Could not find the engine source to compile
/// (searched up from ~/.renzora/templates/r1-alpha7/linux-x64)
/// ```
///
/// That path was harmless while lean builds were host-only, because the host's
/// template dir *is* `<engine>/dist/<platform>/`. Once a lean build could target
/// another platform it stopped being true — and a lean build never reads the
/// template anyway, since it recompiles the engine from source.
/// Where a plugin's SOURCE lives, for the exporter to compile from.
///
/// Beside the editor, not in the engine checkout. Plugins used to live in the
/// repository's `plugins/`, so `engine_src.join("plugins")` was the same
/// directory and the distinction never came up. They are distributed through the
/// marketplace now and installed into `<editor>/plugins/<id>/`, source and all —
/// which is what a lean export has to compile, and it is also the only copy that
/// exists on a machine that installed the editor rather than cloning it.
///
/// The failure this fixes is quiet in the worst way: the plan finds no source,
/// says so in one line among a hundred compiler lines, and ships every plugin as
/// a loose file — producing a "lean" build that is not lean and a single binary
/// that is not single.
pub fn plugin_source_root() -> Option<PathBuf> {
    let dir = editor_dir()?.join("plugins");
    dir.is_dir().then_some(dir)
}

pub fn editor_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

/// The engine source a lean build compiles: the checkout the editor runs from
/// if there is one, otherwise a downloaded copy under `~/.renzora/src/<version>/`.
///
/// The checkout wins deliberately. A contributor's tree is the one they are
/// editing, and silently compiling a downloaded copy instead would produce a
/// binary that did not contain their changes — the kind of wrong that looks like
/// the build system lying.
///
/// The download exists so a canonical editor — binaries, no source — can still
/// do a lean export. It is fetched on demand by
/// [`crate::download::spawn_source_download`]; `None` here means neither is
/// present, which the UI turns into a Download offer rather than a dead end.
pub fn resolve_engine_source() -> Option<PathBuf> {
    if let Some(checkout) = editor_dir().and_then(|d| find_engine_source(&d)) {
        return Some(checkout);
    }
    let downloaded = crate::templates::user_source_dir()?;
    // Verified by the same signature rather than mere existence: a half-extracted
    // or emptied directory would otherwise be handed to cargo, which fails much
    // later and much less clearly.
    is_engine_source(&downloaded).then_some(downloaded)
}

/// Does this directory look like the engine workspace root?
///
/// `Cargo.toml` + `crates/` + `src/main.rs` together, so a sub-crate's manifest
/// cannot be mistaken for the root.
fn is_engine_source(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file()
        && dir.join("crates").is_dir()
        && dir.join("src").join("main.rs").is_file()
}

pub fn find_engine_source(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join("Cargo.toml").is_file()
            && d.join("crates").is_dir()
            && d.join("src").join("main.rs").is_file()
        {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Compile a lean static `renzora` binary for `platform` from the engine source
/// at `workspace_dir` (the engine checkout, NOT the project). Returns the path to
/// the freshly built binary; the caller embeds the project's rpak into it.
///
/// `static_plugins` is the set the user chose to compile **into** the binary
/// rather than ship as loose libraries beside it (see [`stage_static_plugins`]).
/// Empty is the default and means the same as it always did: the caller copies
/// the selected plugins into `plugins/` and the host `dlopen`s them, which works
/// against a static binary because a C-ABI plugin links no Bevy and so has
/// nothing to share with the host.
///
/// Three build shapes, decided by the platform:
///
/// * **the host** — native `toolchain` cargo, no `--target`.
/// * **the web** — native `toolchain` cargo *with* `--target wasm32-unknown-
///   unknown`. Another architecture, but not another OS: rustc carries the
///   linker and (once `rustup target add` has run) the std, so no container is
///   involved. The caller still has to run the bindgen chain over the result —
///   see [`crate::wasm`].
/// * **another OS** — that platform's toolchain container, which is where the
///   cross-linker and its system libraries live.
#[allow(clippy::too_many_arguments)]
pub fn build_lean(
    workspace_dir: &Path,
    // The project being exported — where its `scripts/*.rs` are read from. Not
    // the engine source, which `workspace_dir` points at.
    project_dir: &Path,
    platform: Platform,
    // `None` only for a container build, which carries its own compiler. The web
    // needs one despite not being the host.
    toolchain: Option<&Toolchain>,
    progress: &mut dyn FnMut(String),
    disabled_bevy_features: &[String],
    disabled_runtime_features: &[String],
    profile: LeanProfile,
    static_plugins: &[StaticPluginSrc],
    // The executable's icon and version-info strings. Compile-time only — see
    // [`stage_branding`].
    branding: &LeanBranding,
    cancel: &Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    // ── Docker only for another OS; the host builds natively ─────────────────
    //
    // A container is a cross-compiler, and there is nothing to cross-compile for
    // the machine you are sitting at. Native is also faster (no image pull, no
    // bind mount) and needs no Docker install at all — so someone exporting for
    // their own platform never has to have it.
    //
    // **"In a container" and "passes `--target`" are two questions, not one.**
    // They coincided for as long as every lean target was a desktop OS, and the
    // web breaks that: `wasm32-unknown-unknown` needs `--target` (it is not the
    // host arch) but not a container (rustc ships its linker and std). Conflating
    // them sent the web build to Docker, which does not even have an image for
    // it. Keep the two `let`s below distinct.
    //
    // Checked up front, before the workspace sync copies the engine source —
    // that takes long enough to look like the build had already started.
    let in_container = crate::docker::needs_container(platform);
    // `Some` whenever cargo must be told the target explicitly: every container
    // build, plus the web on the host.
    let target_triple: Option<&str> = if in_container || matches!(platform, Platform::WebWasm32) {
        Some(crate::docker::rust_triple(platform).ok_or_else(|| {
            format!("No Rust target for {}", platform.display_name())
        })?)
    } else {
        None
    };
    if in_container {
        if !crate::docker::lean_supported(platform) {
            return Err(format!(
                "No lean build is available for {} — it has no toolchain image.",
                platform.display_name(),
            ));
        }
        match crate::docker::probe() {
            crate::docker::DockerStatus::Ready => {}
            crate::docker::DockerStatus::NotInstalled => {
                return Err(format!(
                    "A lean build for {} is compiled in a container, so it needs Docker. \
                     Install it from {} and try again.",
                    platform.display_name(),
                    crate::docker::INSTALL_URL,
                ));
            }
            crate::docker::DockerStatus::NotRunning(why) => {
                return Err(format!(
                    "Docker is installed but not responding, so the {} build cannot start. \
                     Start Docker Desktop and try again.\n{why}",
                    platform.display_name(),
                ));
            }
        }
    }

    if !workspace_dir.join("Cargo.toml").is_file() {
        return Err(format!(
            "No Cargo.toml at {} — a lean build recompiles the engine, so it needs \
             the engine source checkout.",
            workspace_dir.display()
        ));
    }

    // Build from an ISOLATED COPY of the engine source — never the dev tree, so
    // `cargo renzora` / `renzora run` are completely unaffected. The copy can be
    // patched freely with no restore (it's disposable). It has its own `target/`,
    // so the dev cache and locks are untouched and exports stay incremental
    // across runs.
    let ws = sync_export_workspace(workspace_dir, progress)?;
    if in_container {
        patch_cross_cargo_config(&ws, platform, progress)?;
    }
    stage_branding(workspace_dir, &ws, branding, progress)?;
    strip_bevy_features(&ws, disabled_bevy_features, progress)?;
    strip_runtime_features(&ws, disabled_runtime_features, progress)?;
    patch_lean_profile(&ws, profile, progress)?;
    // The plugins' SOURCE, which is beside the editor rather than in the
    // checkout — `workspace_dir` is the engine repo and has held no `plugins/`
    // since they moved to the marketplace. Empty is not an error here: the plan
    // this stages was resolved from the same root, so an empty root produced an
    // empty plan and there is nothing to stage.
    let plugins_root = plugin_source_root().unwrap_or_else(|| workspace_dir.join("plugins"));
    stage_static_plugins(&plugins_root, &ws, static_plugins, progress)?;
    let has_scripts = stage_static_scripts(project_dir, &ws, progress)?;
    let mut features = String::from(runtime_feature(platform));
    if !static_plugins.is_empty() {
        features.push_str(",static_plugins");
    }
    // Only when there is something to link. Turning it on with an empty table
    // would compile the aggregator and the `renzora/static_scripts` variant of
    // `script!` for nothing.
    if has_scripts {
        features.push_str(",static_scripts");
    }

    // Native cargo for the host; the toolchain container for anything else. The
    // cargo arguments after this are identical either way — a container build is
    // the same build, run somewhere that has the cross-linker.
    let mut cmd = if in_container {
        let image = crate::docker::image_for(platform)
            .ok_or_else(|| format!("No toolchain image for {}", platform.display_name()))?;
        let image_ref = crate::docker::image_ref(workspace_dir, image).ok_or_else(|| {
            format!(
                "Could not read docker/base/Dockerfile and docker/{image}/Dockerfile under {} \
                 to resolve the toolchain image tag.",
                workspace_dir.display()
            )
        })?;
        progress(format!("Building in {image_ref}"));
        let mut c = crate::docker::build_command(&image_ref, &ws);
        c.arg("cargo");
        c
    } else {
        let tc = toolchain.ok_or(
            "Internal error: a native lean build was started without a Rust toolchain.",
        )?;
        // The web is the one native build that targets another architecture, so
        // it is the one that can be missing its standard library. Done here
        // rather than in the caller because this is where the triple is known.
        if let Some(triple) = target_triple {
            crate::toolchain::ensure_target(tc, triple, progress)?;
        }
        let mut c = tc.cargo_command();
        c.current_dir(&ws);
        c
    };
    if !in_container {
        cmd.env("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags(platform));
    }
    cmd.args([
            "build",
            "--profile",
            "dist-lean",
            "--bin",
            "renzora",
            "--no-default-features",
        ])
        .arg("--features")
        .arg(&features);
    if let Some(triple) = target_triple {
        // `--target` selects the cross-linker (or, for the web, the wasm
        // backend) and nests the output under the triple — which the binary path
        // below accounts for.
        cmd.args(["--target", triple]);
    }
    if !in_container && matches!(platform, Platform::LinuxX64) {
        // The repo config pins linker=clang + `-fuse-ld=mold`; a provisioned
        // minimal toolchain may have neither. `cc` is present on essentially
        // every Linux dev host.
        cmd.args(["--config", "target.x86_64-unknown-linux-gnu.linker=\"cc\""]);
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start cargo: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    // Share the child so a watcher thread can kill it on cancel. Only ever held
    // briefly (kill / wait), never across a blocking read, so cancel can't dead-
    // lock against the reaper.
    let child = Arc::new(Mutex::new(child));

    // Watcher: when Cancel is clicked, kill the build. Stops itself once the
    // build is done (the `done` flag the main thread sets after wait()).
    let done = Arc::new(AtomicBool::new(false));
    {
        let cancel = cancel.clone();
        let done = done.clone();
        let child = child.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    if let Ok(mut c) = child.lock() {
                        let _ = c.kill();
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        });
    }

    // Drain stdout on a side thread so a full pipe can't deadlock the build;
    // cargo's human progress goes to stderr, which we forward live and keep a
    // tail of for error reporting. On cancel the watcher kills the child, the
    // pipes hit EOF, and this read loop ends.
    if let Some(out) = stdout {
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = line;
            }
        });
    }

    let mut tail: Vec<String> = Vec::new();
    if let Some(err) = stderr {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            // Surface the most recent cargo line as progress (compiling crate N…).
            progress(line.clone());
            tail.push(line);
            if tail.len() > 60 {
                tail.remove(0);
            }
        }
    }

    let status = child
        .lock()
        .unwrap()
        .wait()
        .map_err(|e| format!("Failed waiting for cargo: {e}"))?;
    done.store(true, Ordering::Relaxed);

    if cancel.load(Ordering::Relaxed) {
        return Err("Export cancelled".into());
    }
    if !status.success() {
        return Err(format!(
            "Lean build failed (cargo exited with {}):\n{}",
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into()),
            tail.join("\n")
        ));
    }

    // `--target` nests the output under the triple; a plain host build has no
    // `--target` and writes straight into `target/<profile>/`. That follows the
    // flag, not the container — which is why the web (native, but targeted)
    // lands in the nested path too.
    let bin = match target_triple {
        Some(triple) => ws
            .join("target")
            .join(triple)
            .join("dist-lean")
            .join(bin_filename(platform)),
        None => ws.join("target").join("dist-lean").join(bin_filename(platform)),
    };
    if !bin.is_file() {
        return Err(format!(
            "Lean build reported success but the binary is missing at {}",
            bin.display()
        ));
    }
    Ok(bin)
}

/// The icon the root `build.rs` embeds into the executable's resource table.
/// The name is not configurable — `build.rs` hardcodes it — so staging a custom
/// icon means writing this exact file into the export copy.
const ICON_FILE: &str = "icon.ico";

/// The version-info overrides the root `build.rs` reads. See `Branding` there.
const BRANDING_FILE: &str = "export-branding.txt";

/// What the exported executable should *say it is* — the parts of the binary
/// that are decided at compile time and so cannot be patched in afterwards.
///
/// Both fields are `None` for a build that wants the engine's own branding,
/// which is what a developer running a plain lean export of an unnamed project
/// gets. There is deliberately no default project name here: a game called
/// "Renzora Engine" in the Properties dialog is wrong, but so is one called
/// "Untitled", and the caller knows which it has.
#[derive(Default, Clone)]
pub struct LeanBranding {
    /// The author's picked icon file, in any format `crate::icon` can decode.
    pub icon: Option<PathBuf>,
    /// Shown as both ProductName and FileDescription in the Properties dialog.
    pub product_name: Option<String>,
}

/// Put the game's icon and version-info strings in front of the compiler.
///
/// Neither can be applied to a finished binary without rewriting its PE resource
/// section, so both have to be staged *before* cargo runs — which is the whole
/// reason this exists as a separate step rather than a post-processing pass.
/// Falls back to the engine's own `icon.ico` when the project has none, so an
/// export never ends up with a blank icon.
///
/// Everything is written only when the bytes actually differ. `build.rs` declares
/// `rerun-if-changed` on both files, so a blind rewrite would rebuild the root
/// crate and relink a 200 MB binary on every export.
fn stage_branding(
    engine_src: &Path,
    ws: &Path,
    branding: &LeanBranding,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    match branding.icon.as_deref() {
        Some(src) => {
            progress(format!("Embedding {} in the executable…", src.display()));
            let base = crate::icon::load_square(src)?;
            write_bytes_if_changed(&ws.join(ICON_FILE), &crate::icon::to_ico(&base)?)?;
        }
        None => {
            // No project icon: mirror the engine's, which is what the copy used
            // to get for free from the root-file sync.
            let engine_icon = engine_src.join(ICON_FILE);
            if let Ok(bytes) = std::fs::read(&engine_icon) {
                write_bytes_if_changed(&ws.join(ICON_FILE), &bytes)?;
            }
        }
    }

    let dest = ws.join(BRANDING_FILE);
    match branding.product_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => write_if_changed(
            &dest,
            &format!("product_name={name}\nfile_description={name}\n"),
        )?,
        // A leftover from an earlier export of a differently-named project would
        // otherwise brand this one, since the file lives in the reused copy.
        None if dest.exists() => std::fs::remove_file(&dest)
            .map_err(|e| format!("remove {}: {e}", dest.display()))?,
        None => {}
    }
    Ok(())
}

/// Byte-level [`write_if_changed`], for the icon (which is not text).
fn write_bytes_if_changed(path: &Path, content: &[u8]) -> Result<(), String> {
    if std::fs::read(path).is_ok_and(|old| old == content) {
        return Ok(());
    }
    std::fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Sync the engine source into an isolated copy that the export build compiles,
/// so the dev tree is NEVER touched (`cargo renzora` / `renzora run` stay
/// pristine). The copy lives under the gitignored `target/` and has its own build
/// cache, so it's both isolated and incremental. Returns the copy's root.
///
/// This is a copy-if-newer mirror (by size + mtime), not a full re-copy: the
/// first export copies everything, later ones only touch changed files. It does
/// NOT delete files removed from the source — deleting a crate then re-exporting
/// without clearing `target/export-src` is the one case that needs a manual
/// clear (rare); everything else just works.
fn sync_export_workspace(
    engine_src: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let dest = engine_src.join("target").join("export-src");
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("create export workspace: {e}"))?;
    progress("Syncing engine source into the isolated export workspace…".into());

    // Top-level dirs that are never part of a build. Matched ONLY at the root, so
    // they can't collide with same-named dirs nested inside `crates/` (e.g. a
    // crate's own `docs/`, or the critical `crates/renzora` vs the stray root
    // `renzora/`). `target` also covers `target/export-src` itself, so the sync
    // can't recurse into its own destination.
    //
    // `plugins` is here because those are separate cargo projects that the
    // engine build never reads — they are only ever needed by the linked-in
    // plugin path, which stages the handful it wants itself (and has to patch
    // their manifests, which a blanket copy-if-newer would keep undoing). See
    // `stage_static_plugins`.
    const TOP_SKIP: &[&str] = &[
        "target", ".git", ".github", ".vscode", ".idea", "dist", "docs",
        "node_modules", "templates", "disabled", "docker", ".claude", ".devcontainer",
        "plugins",
    ];
    // Root FILES `stage_branding` owns outright. Copying the engine's own
    // `icon.ico` in would put the Renzora logo back on the game's executable
    // every time — and because copy-if-newer fires on any size difference, the
    // two would take turns overwriting each other and relink the binary on every
    // single export even when nothing changed.
    const TOP_FILE_SKIP: &[&str] = &[ICON_FILE, BRANDING_FILE];
    // cdylib crates are never linked into the lean binary, so leave them out of
    // the copy entirely.
    let drop_plugins = cdylib_crates(engine_src);
    let mut copied = 0usize;
    for entry in std::fs::read_dir(engine_src).map_err(|e| format!("read {}: {e}", engine_src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let s = entry.path();
        let d = dest.join(&name);
        if ft.is_dir() {
            if TOP_SKIP.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            std::fs::create_dir_all(&d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
            if name.to_string_lossy() == "crates" {
                sync_crates(&s, &d, &drop_plugins, &mut copied)?;
            } else {
                sync_dir(&s, &d, &mut copied)?;
            }
        } else if ft.is_file() {
            if TOP_FILE_SKIP.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            if should_copy(&s, &d) {
                std::fs::copy(&s, &d).map_err(|e| format!("copy {}: {e}", s.display()))?;
                copied += 1;
            }
        }
    }
    if !drop_plugins.is_empty() {
        progress(format!(
            "Excluding {} unused distribution plugin(s) from the copy",
            drop_plugins.len()
        ));
    }
    progress(format!("Export workspace ready ({copied} file(s) updated)"));
    Ok(dest)
}

/// Sync `crates/`, but skip (and prune from the copy) the cdylib crates — they
/// are never linked into the lean binary, so copying and resolving them is pure
/// waste. Everything else (core rlib crates, vendored crates) syncs normally.
fn sync_crates(
    src: &Path,
    dest: &Path,
    drop_plugins: &HashSet<String>,
    copied: &mut usize,
) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let s = entry.path();
        let d = dest.join(&name);
        // Wholly written by `stage_static_plugins`. Copying the checked-in stub
        // over the generated version would undo it — and since the two differ in
        // size, the copy-if-newer test would fire on every export and rebuild the
        // aggregator (and relink the binary) whether or not anything changed.
        if name_str == "renzora_static_plugins" {
            continue;
        }
        if ft.is_dir() && drop_plugins.contains(name_str.as_ref()) {
            // Pruned: ensure it's absent (a prior export with a different plugin
            // selection may have copied it).
            if d.exists() {
                let _ = std::fs::remove_dir_all(&d);
            }
            continue;
        }
        if ft.is_dir() {
            std::fs::create_dir_all(&d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
            sync_dir(&s, &d, copied)?;
        } else if ft.is_file() && should_copy(&s, &d) {
            std::fs::copy(&s, &d).map_err(|e| format!("copy {}: {e}", s.display()))?;
            *copied += 1;
        }
    }
    Ok(())
}

/// Names of `crates/` entries that build a cdylib — never in the lean binary's
/// link closure, so the copy skips them. Core runtime subsystems are rlib
/// libraries (no `cdylib` crate-type) and never match. The game's own C-ABI
/// plugins live in `plugins/`, not here — they either ship as files beside the
/// binary or are linked in via `stage_static_plugins`, which stages them itself.
fn cdylib_crates(engine_src: &Path) -> HashSet<String> {
    let mut drop = HashSet::new();
    let Ok(rd) = std::fs::read_dir(engine_src.join("crates")) else {
        return drop;
    };
    for entry in rd.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Ok(text) = std::fs::read_to_string(entry.path().join("Cargo.toml")) {
            if is_cdylib_crate(&text) {
                drop.insert(name);
            }
        }
    }
    drop
}

/// Whether a manifest declares a `cdylib` crate-type (a distribution plugin /
/// bundle), as opposed to an rlib core library or the `dylib`+rlib `renzora`
/// contract. Ignores commented-out lines.
fn is_cdylib_crate(manifest: &str) -> bool {
    manifest.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("crate-type") && l.contains("cdylib")
    })
}

/// Recursive copy-if-newer of `src` → `dest`. Inside the tree only build/vcs
/// noise (never source) is skipped, so no needed crate file is missed.
fn sync_dir(src: &Path, dest: &Path, copied: &mut usize) -> Result<(), String> {
    const DEEP_SKIP: &[&str] = &["target", ".git"];
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let s = entry.path();
        let d = dest.join(&name);
        if ft.is_dir() {
            if DEEP_SKIP.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            std::fs::create_dir_all(&d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
            sync_dir(&s, &d, copied)?;
        } else if ft.is_file() && should_copy(&s, &d) {
            std::fs::copy(&s, &d).map_err(|e| format!("copy {}: {e}", s.display()))?;
            *copied += 1;
        }
    }
    Ok(())
}

/// Whether `src` should be copied over `dest`: missing, different size, or newer.
fn should_copy(src: &Path, dest: &Path) -> bool {
    let (Ok(sm), Ok(dm)) = (std::fs::metadata(src), std::fs::metadata(dest)) else {
        return true;
    };
    if sm.len() != dm.len() {
        return true;
    }
    match (sm.modified(), dm.modified()) {
        (Ok(st), Ok(dt)) => st > dt,
        _ => true,
    }
}

/// Strip `disabled` Bevy features from the export copy's root `Cargo.toml`
/// (the `[workspace.dependencies] bevy` feature list), so the lean binary doesn't
/// compile capabilities the game doesn't use. Safe because it edits the copy, not
/// the dev source. Format-preserving via `toml_edit`. No-op if `disabled` empty.
fn strip_bevy_features(
    copy_root: &Path,
    disabled: &[String],
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    if disabled.is_empty() {
        return Ok(());
    }
    let manifest = copy_root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    let mut removed = 0usize;
    if doc.get("workspace").is_some() {
        removed += trim_bevy_features(doc.get_mut("workspace"), disabled);
    }
    if doc.get("target").is_some() {
        if let Some(target) = doc.get_mut("target").and_then(|t| t.as_table_like_mut()) {
            for (_cfg, item) in target.iter_mut() {
                removed += trim_bevy_features(Some(item), disabled);
            }
        }
    }
    if removed == 0 {
        return Ok(());
    }
    std::fs::write(&manifest, doc.to_string())
        .map_err(|e| format!("write {}: {e}", manifest.display()))?;
    progress(format!("Stripping {removed} unused Bevy feature(s)"));
    Ok(())
}

/// Trim `disabled` from `table`'s `dependencies.bevy.features`, and report how
/// many entries went. A table with no such list is left completely alone.
///
/// # Why there is more than one such table
///
/// `[workspace.dependencies].bevy` holds the main feature list, and
/// `[target.'cfg(not(target_arch = "wasm32"))'.dependencies].bevy` holds a
/// second — the features the web build cannot have, kept apart so the wasm lane
/// simply never turns them on. Trimming only the first meant three capabilities
/// silently did nothing on every desktop export: "Editor/dev conveniences"
/// removed `file_watcher`, `system_clipboard` and `clipboard_image` from the
/// workspace list and the target block put all three straight back, so
/// `bevy_clipboard`, `arboard` and a second copy of the `image` decoder stack
/// shipped in games whose author had explicitly asked for none of it. The four
/// `pbr_*_textures` entries sat the same way under "Advanced PBR texture maps",
/// and `basis-universal` under the image decoders.
///
/// # Why existence is checked with the immutable `get` first
///
/// Because `toml_edit`'s `get_mut` **creates** a missing key. Walking
/// `dependencies.bevy.features` mutably through a target block that has no
/// `bevy` entry does not simply return `None` — it inserts one, and the manifest
/// gains a bare `bevy = {}` that cargo refuses outright:
///
/// ```text
/// dependency (bevy) specified without providing a local path, Git repository,
/// version, or workspace dependency to use
/// ```
///
/// The root manifest has exactly such a block (`cfg(target_arch = "wasm32")`,
/// for the getrandom backends), so the first version of this walk broke every
/// export the moment it shipped. The immutable pre-check is the whole fix.
fn trim_bevy_features(table: Option<&mut toml_edit::Item>, disabled: &[String]) -> usize {
    let Some(table) = table else { return 0 };
    let present = table
        .get("dependencies")
        .and_then(|d| d.get("bevy"))
        .and_then(|b| b.get("features"))
        .and_then(|f| f.as_array())
        .is_some();
    if !present {
        return 0;
    }
    let Some(arr) = table
        .get_mut("dependencies")
        .and_then(|d| d.get_mut("bevy"))
        .and_then(|b| b.get_mut("features"))
        .and_then(|f| f.as_array_mut())
    else {
        return 0;
    };
    let before = arr.len();
    arr.retain(|v| {
        v.as_str()
            .map(|s| !disabled.iter().any(|d| d == s))
            .unwrap_or(true)
    });
    before - arr.len()
}

/// The `[profile.dist-lean]` knobs the export UI can move.
///
/// All three are size-for-something trades that the dev tree deliberately does
/// NOT take (see the profile's notes in the root `Cargo.toml`), so they live
/// here as per-export choices rather than as edits to the checked-in profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeanProfile {
    /// `panic = "abort"` — trades fault isolation for ~24% off the binary.
    pub panic_abort: bool,
    /// `opt-level = "z"` instead of the profile's `"s"`. Same as `s` but with
    /// loop vectorization off too.
    pub opt_level_z: bool,
    /// `codegen-units = 1` instead of the release default of 16 — one LLVM
    /// module per crate, so thin LTO has fewer duplicated inline copies to
    /// merge, at a large cost in build time.
    pub codegen_units_one: bool,
}

/// Patch the export copy's `[profile.dist-lean]` to match `opts`.
///
/// `panic = "abort"` is the largest single lever there is — measured 60.9 MB →
/// 46.7 MB on a cube-and-light project, because dropping unwinding removes the
/// landing pads and cleanup glue from `.text` and the panic message/location
/// strings from `.rdata`, not merely the `.pdata` unwind tables. It is only legal
/// because the copy builds `renzora` as `rlib` only (see
/// `sync_export_workspace`); the dev tree's `dylib` would link the precompiled
/// std's `panic_unwind` and refuse to mix strategies.
///
/// Every key is written on every export, never left to whatever the last one
/// set: the copy persists between exports, so a stale `panic = "abort"` or
/// `codegen-units = 1` would silently apply to a later export that had switched
/// the toggle back off. `opt-level` is *set* to `"s"` rather than removed for
/// the same reason in reverse — removing it would inherit `dist`'s `opt-level =
/// 2`, which is a speed profile, whereas absent `codegen-units` correctly means
/// the release default of 16.
fn patch_lean_profile(
    copy_root: &Path,
    opts: LeanProfile,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let manifest = copy_root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    let Some(profile) = doc
        .get_mut("profile")
        .and_then(|p| p.get_mut("dist-lean"))
        .and_then(|p| p.as_table_like_mut())
    else {
        return Ok(());
    };

    if opts.panic_abort {
        profile.insert("panic", toml_edit::value("abort"));
    } else {
        profile.remove("panic");
    }
    profile.insert(
        "opt-level",
        toml_edit::value(if opts.opt_level_z { "z" } else { "s" }),
    );
    if opts.codegen_units_one {
        profile.insert("codegen-units", toml_edit::value(1i64));
    } else {
        profile.remove("codegen-units");
    }

    // Only rewrite when something actually changed: an untouched manifest keeps
    // its mtime, and cargo fingerprints the whole workspace manifest set — a
    // gratuitous write would rebuild every crate on an otherwise no-op export.
    write_if_changed(&manifest, &doc.to_string())?;

    if opts.panic_abort {
        progress("Building with panic = abort (no unwinding)".to_string());
    }
    if opts.opt_level_z {
        progress("Building with opt-level = z (no loop vectorization)".to_string());
    }
    if opts.codegen_units_one {
        progress("Building with codegen-units = 1 (slower, smaller)".to_string());
    }
    Ok(())
}

/// Strip `disabled` subsystem features from the export copy's
/// `renzora_runtime/Cargo.toml` `[features] default`, so a game that doesn't use
/// (e.g.) the sky or post-FX subsystems doesn't compile/register them. Safe: it
/// edits the copy, not the dev source. No-op if `disabled` empty.
fn strip_runtime_features(
    copy_root: &Path,
    disabled: &[String],
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    if disabled.is_empty() {
        return Ok(());
    }
    let manifest = copy_root
        .join("crates")
        .join("renzora_runtime")
        .join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    let arr = doc
        .get_mut("features")
        .and_then(|f| f.get_mut("default"))
        .and_then(|d| d.as_array_mut());
    let Some(arr) = arr else {
        return Ok(());
    };
    arr.retain(|v| {
        v.as_str()
            .map(|s| !disabled.iter().any(|d| d == s))
            .unwrap_or(true)
    });
    std::fs::write(&manifest, doc.to_string())
        .map_err(|e| format!("write {}: {e}", manifest.display()))?;
    progress(format!("Stripping {} unused subsystem(s)", disabled.len()));
    Ok(())
}

// ── Statically-linked plugins ────────────────────────────────────────────────

/// Which mechanism a linked-in plugin belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticKind {
    /// `crate-type = ["cdylib"]`, registered with `renzora_plugin::add!`.
    /// Installed as a `StaticPlugin` entry the host calls across the ABI.
    CAbi,
    /// `crate-type = ["dylib"]`, declared with `renzora::plugin!(Expr, Scope)`.
    /// Carries that expression, because installing it is
    /// `app.add_plugins(<crate>::<expr>)` and there is nothing else to call.
    Native { expr: String },
}

/// One plugin to compile into the binary instead of shipping beside it.
///
/// [`resolve_static_plugins`] builds these by pairing what the export UI listed
/// — which comes from scanning built libraries in `dist/<platform>/plugins/` —
/// with the source directory that produced each one, because linking a plugin in
/// means compiling it, not copying it.
#[derive(Debug, Clone)]
pub struct StaticPluginSrc {
    /// The plugin's crate name — the Rust identifier the generated list names,
    /// and what the host logs the plugin as. Underscored, because that is what
    /// rustc sees; see `package` for what cargo is told.
    pub id: String,
    /// How this plugin is installed once it is linked in.
    ///
    /// Not cosmetic: the two are compiled the same way and installed by entirely
    /// different code. A C-ABI plugin becomes a `StaticPlugin` the host calls
    /// through; a native one is an ordinary Bevy plugin, so the generated crate
    /// has to `add_plugins` its type by name.
    pub kind: StaticKind,
    /// The `[package] name` as written, which may contain dashes. Cargo resolves
    /// a dependency by this and rustc then substitutes underscores, so the
    /// generated manifest must use it verbatim and the generated code must not.
    pub package: String,
    /// The library stem the export UI keys its selection on. Differs from `id`
    /// on Unix, where a cdylib is `lib<crate>.so`, so the two are kept apart:
    /// the caller matches its selection on THIS, and a filter written against
    /// `id` would let every linked plugin be copied beside the binary as well.
    pub library_stem: String,
    /// Its directory under `plugins/`. Usually the same as `id`, but the two are
    /// resolved separately because a package name need not match its folder.
    pub dir: String,
    /// `true` for an Editor-scope plugin. Recorded rather than filtered so the
    /// host applies the same scope rule to a linked plugin as to a loaded one.
    pub editor_scope: bool,
}

/// What a lean build will actually do with the plugins the user ticked.
///
/// Three outcomes rather than two, because "cannot be linked in" splits into two
/// situations that deserve different answers and different words.
pub struct StaticPluginPlan {
    /// Compiled into the binary.
    pub linked: Vec<StaticPluginSrc>,
    /// No source in this checkout, so nothing to compile. Shipped as a file
    /// beside the binary instead — which is a perfectly good outcome on desktop,
    /// and the reason this is not an error.
    pub no_source: Vec<String>,
    /// The source IS here, and the plugin says it cannot be built for this
    /// target. Left out entirely: on the web there is no file to fall back to,
    /// and on desktop a plugin that declares itself unbuildable would only fail
    /// the compile a few minutes later.
    pub unsupported: Vec<String>,
}

/// Pair each wanted plugin id with the source directory that builds it, and drop
/// the ones that cannot be built for `target_triple`.
///
/// # Why a plugin can be unbuildable for a target
///
/// Most of `plugins/` is pure Rust with no dependencies at all and goes
/// anywhere. A few are not: `audio` is built on cpal and **has no entry point on
/// wasm** (`renzora_plugin_init` is behind `#[cfg(not(target_arch = "wasm32"))]`,
/// awaiting a WebAudio backend), `lua` and `tracy` compile C through `cc` and
/// wasm32-unknown-unknown has no libc sysroot for it, and `http` is built on a
/// socket stack a browser does not have.
///
/// Without this filter the export ticks along happily — syncing the workspace,
/// stripping features, compiling for minutes — and then dies inside the
/// GENERATED aggregator with a message about a symbol in a crate the user never
/// asked about:
///
/// ```text
/// error[E0425]: cannot find value `renzora_plugin_init` in crate `audio`
/// note: found an item that was configured out
/// ```
///
/// A plugin declares this itself, because the plugin author is the one who
/// knows, in its own `Cargo.toml`:
///
/// ```toml
/// [package.metadata.renzora]
/// unsupported-targets = ["wasm32"]
/// ```
///
/// Each entry is matched as a **substring of the Rust target triple**, so
/// `"wasm32"` covers `wasm32-unknown-unknown` and `"apple-darwin"` would cover
/// both macOS arches. Absent means "builds anywhere", which is true of nearly
/// every plugin and is why the default has to be the permissive one.
pub fn resolve_static_plugins(
    // Directory holding one source directory per plugin — see
    // `plugin_source_root`. Named `plugins_root` rather than `engine_src`
    // because it is no longer inside the checkout.
    plugins_root: &Path,
    wanted: &[(String, bool)],
    // `None` for a host build with no `--target`, which is the host triple by
    // definition; a plugin cannot be unsupported on the platform whose editor is
    // listing it, so nothing is filtered.
    target_triple: Option<&str>,
) -> StaticPluginPlan {
    // Package name → directory, read from the manifests rather than assumed from
    // the folder names: the library a plugin produces is named after its
    // `[package] name` (with dashes underscored), and that is what the scan sees.
    // Underscored package name → (package name as written, directory, buildable).
    let mut by_package: std::collections::HashMap<String, (String, String, bool, StaticKind)> =
        Default::default();
    if let Ok(entries) = std::fs::read_dir(plugins_root) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let dir = entry.file_name().to_string_lossy().into_owned();
            let Ok(text) = std::fs::read_to_string(entry.path().join("Cargo.toml")) else {
                continue;
            };
            if let Some(name) = package_name(&text) {
                let buildable = match target_triple {
                    Some(triple) => supports_target(&text, triple),
                    None => true,
                };
                // `crate-type` is the distinction everywhere else in the engine,
                // so it is the distinction here. A `dylib` is native and needs
                // its `plugin!` expression read out of the source; a `cdylib` is
                // C-ABI and is described entirely by its manifest.
                let kind = if declares_dylib(&text) {
                    match native_plugin_expr(&entry.path()) {
                        Some(expr) => Some(StaticKind::Native { expr }),
                        // Declared no plugin, or declared one this cannot name —
                        // `plugin!(MyPlugin { size: 4 }, Runtime)` is legal and
                        // is not a path. Left unlinked and shipped as a file
                        // instead, which is what a copy-based export does anyway.
                        None => None,
                    }
                } else {
                    Some(StaticKind::CAbi)
                };
                if let Some(kind) = kind {
                    by_package.insert(name.replace('-', "_"), (name, dir, buildable, kind));
                }
            }
        }
    }

    let mut plan = StaticPluginPlan {
        linked: Vec::new(),
        no_source: Vec::new(),
        unsupported: Vec::new(),
    };
    for (id, editor_scope) in wanted {
        // Unix libraries are `lib<crate>.so`; the scan keeps the stem verbatim,
        // so strip the prefix before matching a crate name against it.
        let crate_name = id.strip_prefix("lib").unwrap_or(id.as_str());
        match by_package
            .get(id.as_str())
            .or_else(|| by_package.get(crate_name))
        {
            Some((_, _, false, _)) => plan.unsupported.push(crate_name.to_string()),
            Some((package, dir, true, kind)) => plan.linked.push(StaticPluginSrc {
                id: crate_name.to_string(),
                kind: kind.clone(),
                package: package.clone(),
                library_stem: id.clone(),
                dir: dir.clone(),
                editor_scope: *editor_scope,
            }),
            None => plan.no_source.push(id.clone()),
        }
    }
    plan.linked.sort_by(|a, b| a.id.cmp(&b.id));
    plan.unsupported.sort();
    plan
}

/// Does this manifest declare a `dylib` — a native, Bevy-linking plugin?
///
/// Checks the quoted `"dylib"`: `"cdylib"` also ends in `dylib`, and matching
/// loosely would call every C-ABI plugin native.
fn declares_dylib(manifest: &str) -> bool {
    manifest
        .lines()
        .filter(|l| l.trim_start().starts_with("crate-type"))
        .any(|l| l.contains("\"dylib\""))
}

/// The expression a native plugin passes to `renzora::plugin!`.
///
/// `plugin!` takes an expression rather than a type — `Box::new($plugin)` — so
/// `plugin!(SplinePlugin, Runtime)` names a unit struct's value. Linking one in
/// is `app.add_plugins(spline::SplinePlugin)`, which needs that text prefixed
/// with the crate name, and that is only sound when the expression is a bare
/// identifier.
///
/// Anything else — `MyPlugin::default()`, `MyPlugin { size: 4 }` — returns
/// `None` rather than being pasted after a `::` to produce a syntax error inside
/// generated code the author never wrote. Such a plugin ships as a file instead.
fn native_plugin_expr(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("src").join("lib.rs")).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        let Some(rest) = t.split_once("plugin!(").map(|(_, r)| r) else { continue };
        // Only `renzora::plugin!` and a bare `plugin!`, never `__native_plugin_entry!`
        // or another crate's macro that happens to end in the same characters.
        let head = t.split("plugin!(").next().unwrap_or_default().trim_end_matches('!');
        if !(head.is_empty() || head.ends_with("renzora::") || head == "plugin") {
            continue;
        }
        let expr = rest.split([',', ')']).next()?.trim();
        let is_ident = !expr.is_empty()
            && expr.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && expr.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if is_ident {
            return Some(expr.to_string());
        }
    }
    None
}

/// Every plugin under `plugins/` that declares it cannot build for `triple`.
///
/// For the export dialog, which lists plugins before anything is compiled and
/// would otherwise let someone tick one that the build then leaves out. Reading
/// the manifests is a few dozen small file reads, so call it when the selected
/// platform changes — not per frame.
pub fn unsupported_plugins_for(plugins_root: &Path, triple: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(plugins_root) else {
        return out;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path().join("Cargo.toml")) else {
            continue;
        };
        if let Some(name) = package_name(&text) {
            if !supports_target(&text, triple) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

/// Whether a plugin manifest allows being built for `triple`.
///
/// Reads `[package.metadata.renzora] unsupported-targets`, a list of substrings
/// matched against the Rust target triple. Anything unparseable or absent means
/// yes: the permissive answer has to be the default, because nearly every plugin
/// builds anywhere and none of them will ever carry this key.
fn supports_target(manifest: &str, triple: &str) -> bool {
    let Ok(doc) = manifest.parse::<toml_edit::DocumentMut>() else {
        return true;
    };
    doc.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("renzora"))
        .and_then(|r| r.get("unsupported-targets"))
        .and_then(|t| t.as_array())
        .map(|list| {
            !list
                .iter()
                .filter_map(|v| v.as_str())
                .any(|pat| triple.contains(pat))
        })
        // No key at all: builds anywhere.
        .unwrap_or(true)
}

/// The `[package] name` of a manifest, without pulling in a full parse.
fn package_name(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find(|l| l.trim_start().starts_with("name = "))
        .and_then(|l| l.split('"').nth(1))
        .map(str::to_string)
}

/// Write the generated `renzora_static_plugins` crate and make each linked
/// plugin buildable as a dependency of it.
///
/// Called on EVERY lean build, including ones linking nothing — the export
/// workspace persists between exports, so a list left over from a previous run
/// would otherwise link plugins the user had since unticked, and the binary
/// would contain code no setting in front of them explained.
///
/// Three edits per plugin, all to the disposable copy:
///
/// 1. **`crate-type` → `rlib`.** A `cdylib` cannot be a Rust dependency; cargo
///    refuses with "found staticlib/cdylib, expected rlib". The cdylib artifact
///    is also pure waste here, so it is replaced rather than appended to.
/// 2. **Drop `[workspace]`.** Each plugin declares itself a workspace root so a
///    standalone `cargo build` in its folder does not inherit the engine's
///    feature unification. As a path dependency that is fatal — cargo reports
///    "multiple workspace roots found in the same workspace" and stops. The root
///    manifest's `exclude = ["plugins"]` keeps them out of the member set anyway,
///    so removing the marker changes nothing except that this now resolves.
/// 3. **Drop `[profile.*]`.** Profiles outside a workspace root are ignored with
///    a warning, and sixty of those warnings buries the build log.
fn stage_static_plugins(
    plugins_root: &Path,
    copy_root: &Path,
    plugins: &[StaticPluginSrc],
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let crate_dir = copy_root.join("crates").join("renzora_static_plugins");
    std::fs::create_dir_all(crate_dir.join("src"))
        .map_err(|e| format!("create {}: {e}", crate_dir.display()))?;

    let mut copied = 0usize;
    for p in plugins {
        let src = plugins_root.join(&p.dir);
        let dest = copy_root.join("plugins").join(&p.dir);
        std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
        // Everything but the manifest, which is written patched below. A plugin
        // that stops being linked leaves its copy behind; nothing references it,
        // and deleting directories a later export may want back is a worse
        // trade than a few hundred KB in a disposable tree.
        sync_dir_except(&src, &dest, "Cargo.toml", &mut copied)?;
        patch_plugin_manifest(&src, &dest.join("Cargo.toml"))?;
    }

    let mut deps = String::new();
    let mut entries = String::new();
    let mut native_calls = String::new();
    for p in plugins {
        deps.push_str(&format!(
            "{name} = {{ path = \"../../plugins/{dir}\" }}\n",
            name = p.package,
            dir = p.dir
        ));
        match &p.kind {
            // Data: the host calls this through the ABI, so it can be described
            // by a struct literal.
            StaticKind::CAbi => entries.push_str(&format!(
                "        StaticPlugin {{\n\
                 \x20           id: \"{id}\",\n\
                 \x20           scope: PluginScope::{scope},\n\
                 \x20           init: {id}::renzora_plugin_init,\n\
                 \x20       }},\n",
                id = p.id,
                scope = if p.editor_scope { "Editor" } else { "Runtime" },
            )),
            // Code: a native plugin is an ordinary `impl Plugin`, and the only
            // way to install one is to hand `add_plugins` its type. There is no
            // symbol to look up and nothing to describe.
            //
            // Editor-scope ones are not emitted at all. A lean binary is a game;
            // an editor plugin compiled into one would run its editor systems
            // there, which is worse than the file being absent.
            StaticKind::Native { expr } if !p.editor_scope => native_calls.push_str(&format!(
                "    app.add_plugins({krate}::{expr});\n",
                krate = p.id,
            )),
            StaticKind::Native { .. } => {}
        }
    }

    let manifest = format!(
        "# GENERATED by the lean exporter — see `renzora_export::build`.\n\
         # Lists the C-ABI plugins compiled into this build's binary.\n\
         [package]\n\
         name = \"renzora_static_plugins\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\
         \n\
         [dependencies]\n\
         # `static_link` strips `#[no_mangle]` from what `renzora_plugin::add!`\n\
         # emits. Without it every plugin below defines `renzora_plugin_init` and\n\
         # the binary fails to link. Cargo unifies features per package, so\n\
         # naming it once here applies it to all of them.\n\
         renzora_plugin = {{ path = \"../renzora_plugin\", features = [\"static_link\"] }}\n\
         # The native counterpart. `plugin!` emits `#[no_mangle]`\n\
         # `renzora_native_plugin_ctor`, and fifty of those in one binary do not\n\
         # link either. Cargo unifies features per package, so naming it here\n\
         # turns it off for every native plugin in the build.\n\
         renzora = {{ path = \"../renzora\", features = [\"static_plugins\"] }}\n\
         bevy = {{ workspace = true }}\n\
         {deps}"
    );
    write_if_changed(&crate_dir.join("Cargo.toml"), &manifest)?;

    let body = if plugins.is_empty() {
        "    Vec::new()\n".to_string()
    } else {
        format!("    vec![\n{entries}    ]\n")
    };
    let lib = format!(
        "//! GENERATED by the lean exporter — see `renzora_export::build`.\n\
         //!\n\
         //! The plugins this build compiled in rather than shipping as files.\n\
         //! Overwritten on every lean export; edits here do not survive one.\n\
         \n\
         use renzora_plugin::static_link::StaticPlugin;\n\
         use renzora_plugin::sys::PluginScope;\n\
         \n\
         pub fn plugins() -> Vec<StaticPlugin> {{\n{body}}}\n\
         \n\
         #[allow(unused_variables)]\n\
         pub fn native_plugins(app: &mut bevy::app::App) {{\n{native_calls}}}\n"
    );
    write_if_changed(&crate_dir.join("src").join("lib.rs"), &lib)?;

    if !plugins.is_empty() {
        progress(format!(
            "Linking {} plugin(s) into the binary: {}",
            plugins.len(),
            plugins
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(())
}

/// Write only when the content differs, so an unchanged plugin selection does
/// not touch the mtime and force cargo to rebuild the aggregator (and relink the
/// whole binary) on every export.
/// Generate the crate that compiles the project's `.rs` scripts into the binary.
///
/// A lean binary links Bevy statically, so there is no shared image for a script
/// dylib to bind to — `RustScriptPlugin` refuses to register a backend and the
/// exported game reports `No backend for Some("rs")`. Compiling the sources in
/// removes the boundary rather than trying to make it safe: no library, no symbol
/// lookup, no second `World` type, because the script is part of the same
/// compilation as everything it touches.
///
/// Each script becomes a `#[path]` module of `renzora_static_scripts`, which is
/// what keeps fifty of them from colliding. Two things would otherwise collide:
///
/// - `renzora::script!` emits `#[unsafe(no_mangle)] fn renzora_script_update`,
///   and fifty of those do not link. The `renzora/static_scripts` feature that
///   the generated manifest turns on drops the attribute; the aggregator then
///   names each entry point by path instead of by symbol.
/// - a script's own items (`#[derive(Component)] struct Spin`) would clash with
///   another script's. Modules namespace them, so two scripts may both define a
///   `Spin` and neither knows.
///
/// Returns whether anything was generated, so the caller only adds the feature
/// when there is something to link.
/// Ship the plugin SDK so the exported game can compile plugins of its own.
///
/// This is what "enable modding" buys. Without it a game loads only the
/// prebuilt libraries the export staged; with it, a player can drop a native
/// plugin's SOURCE into `plugins/` and the game builds it on next launch,
/// exactly as the editor does — same compiler driver, same SDK, same loading.
///
/// Copied rather than repacked. A release ships `sdk.tar.zst` and unpacks it on
/// first run, deleting the archive, so an editor that has been started once has
/// only the extracted tree — and repacking 1.5 GB at `zstd -19` would add
/// minutes to every export to save space in a directory the player never
/// downloads over a network. Whichever form is present is what ships: the
/// archive if the editor has not unpacked it yet, the tree otherwise, and the
/// game's own first-run step handles the archive case.
///
/// Host platform only. An SDK is only correct on the platform it was built for —
/// its proc-macro dylibs belong to whatever ran the compiler — so shipping this
/// editor's SDK inside a game for another OS would hand a player a compiler that
/// cannot run. That is the same rule that makes a cross-built editor unable to
/// compile scripts.
pub fn stage_modding_sdk(
    editor_dir: &Path,
    output_dir: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<bool, String> {
    // Somewhere to put a mod, even when the game shipped no plugins of its own.
    // An empty directory is the instruction: a player who opens the folder can
    // see where a plugin goes, where otherwise they would have to know to create
    // it — and a game with modding enabled and no `plugins/` anywhere looks like
    // modding was not enabled at all.
    let plugins = output_dir.join("plugins");
    std::fs::create_dir_all(&plugins)
        .map_err(|e| format!("create {}: {e}", plugins.display()))?;

    // The archive first: smaller, and the game unpacks it on first launch behind
    // the same progress window the editor uses.
    let archive = editor_dir.join("sdk.tar.zst");
    if archive.is_file() {
        std::fs::copy(&archive, output_dir.join("sdk.tar.zst"))
            .map_err(|e| format!("copy sdk.tar.zst: {e}"))?;
        progress("Shipped the plugin SDK (compressed) for modding".to_string());
        return Ok(true);
    }

    let sdk = editor_dir.join("sdk");
    if !sdk.join("manifest.json").is_file() {
        progress(
            "WARN: modding is on but this editor has no plugin SDK — the game will ship \
             without one and can load only prebuilt plugins."
                .to_string(),
        );
        return Ok(false);
    }

    progress("Copying the plugin SDK for modding (this is ~1.5 GB)…".to_string());
    let copied = copy_dir(&sdk, &output_dir.join("sdk"))?;
    progress(format!("Shipped the plugin SDK for modding ({copied} files)"));
    Ok(true)
}

/// Recursive copy, returning how many files landed.
fn copy_dir(from: &Path, to: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let mut count = 0;
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let src = entry.path();
        let Some(name) = src.file_name() else { continue };
        let dst = to.join(name);
        if src.is_dir() {
            count += copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)
                .map_err(|e| format!("copy {} → {}: {e}", src.display(), dst.display()))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Ship the `Runtime`-scope native plugins the editor already built, beside a
/// copy-based export.
///
/// A native plugin links the real Bevy, so it can only load into a host that
/// shares the same image — which a copy-based export does, since it carries the
/// very `bevy_dylib` and `renzora_dylib` the plugin was compiled against. (A
/// lean export links Bevy statically and shares nothing, so it takes no native
/// plugins at all.)
///
/// **Scope is read from the built library, not the source.** A
/// `plugin!(.., Runtime)` in `src/lib.rs` describes what the source would build
/// to; what ships is the library, and the two disagree whenever one was edited
/// without rebuilding. Asking the artefact removes the discrepancy.
///
/// Only the library is staged — no `src/`, no stamp. The loader treats a
/// directory holding a built library and nothing else as a plugin it can load
/// but not rebuild, which is exactly a shipped game's situation. Shipping the
/// source would put a plugin author's code inside every game that uses it to
/// satisfy a marker nothing reads.
///
/// Host platform only, for the same reason as the scripts: these libraries are
/// host-shaped.
pub fn stage_runtime_native_plugins(
    editor_dir: &Path,
    output_dir: &Path,
    lib_ext: &str,
    // What the exporter's plugin picker has ticked. `None` means the picker
    // never ran, in which case every eligible plugin ships — the behaviour
    // before it listed native plugins at all.
    selected: Option<&std::collections::HashSet<String>>,
    progress: &mut dyn FnMut(String),
) -> Result<usize, String> {
    let src_root = editor_dir.join("plugins");

    // A plugin switched off in Settings → Editor → Plugins must not ship. It is
    // off because the user turned it off, and an export is the last moment that
    // choice can still be honoured — after this it is in a player's hands with
    // no switch at all.
    let disabled = renzora::load_disabled_plugins();

    let mut shipped: Vec<String> = Vec::new();
    let mut skipped_editor: Vec<String> = Vec::new();
    for plugin in renzora_native_plugin::installed(&src_root, lib_ext) {
        let name = plugin.id;
        if disabled.iter().any(|d| d == &name) {
            continue;
        }
        // Unticked in the picker. Distinct from `disabled` above: that is "not
        // in my editor", this is "not in this build".
        if selected.is_some_and(|s| !s.contains(&name)) {
            continue;
        }
        if plugin.scope != renzora::NativePluginScope::Runtime {
            // Editor-only: belongs in no game, and worth saying so.
            skipped_editor.push(name);
            continue;
        }

        let dest = output_dir.join("plugins").join(&name).join("build");
        std::fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
        let dest_lib = dest.join(format!("{}.{lib_ext}", name.replace('-', "_")));
        std::fs::copy(&plugin.lib, &dest_lib)
            .map_err(|e| format!("copy {} → {}: {e}", plugin.lib.display(), dest_lib.display()))?;
        shipped.push(name);
    }

    if !shipped.is_empty() {
        progress(format!(
            "Shipped {} runtime native plugin(s): {}",
            shipped.len(),
            shipped.join(", ")
        ));
    }
    // Said out loud, because "my plugin is missing from the build" is otherwise
    // indistinguishable from a bug — and the fix is one word in the source.
    if !skipped_editor.is_empty() {
        progress(format!(
            "{} native plugin(s) are editor-only and were not shipped ({}). Declare \
             `renzora::plugin!(.., Runtime)` to include one in a game.",
            skipped_editor.len(),
            skipped_editor.join(", ")
        ));
    }
    Ok(shipped.len())
}

/// Ship the script libraries the editor already built, beside a copy-based
/// export.
///
/// A copy-based export carries the same `bevy_dylib` and `renzora_dylib` the
/// editor compiled these against, so the `World` on both sides of the `dlopen`
/// boundary is one type and they load exactly as they do in the editor. The
/// player needs no SDK and no Rust toolchain — the compiling already happened.
///
/// Copies rather than recompiles. The editor builds every script on project open
/// and again on save, so `<project>/.renzora/scripts/<stem>/` already holds a
/// current library; building a second time would only produce the same bytes
/// more slowly. A script that never compiled has nothing there and is reported
/// rather than silently omitted.
///
/// **Host platform only.** These are host-shaped libraries, so they belong with
/// an export for the machine that built them. Staging them into an export for
/// another OS would ship a `.dll` to a Linux player — silently useless. A
/// cross-platform copy export therefore has no Rust scripts, which is a real
/// gap and the reason the lean path compiles them in instead.
///
/// Returns how many were staged.
pub fn stage_prebuilt_scripts(
    project_dir: &Path,
    output_dir: &Path,
    lib_ext: &str,
    progress: &mut dyn FnMut(String),
) -> Result<usize, String> {
    let mut sources: Vec<PathBuf> = Vec::new();
    collect_scripts(project_dir, &mut sources)?;
    sources.sort();
    if sources.is_empty() {
        return Ok(0);
    }

    let dest = output_dir.join("scripts");
    std::fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;

    // Same dual-key scheme the compiled-in path uses, for the same reason: the
    // dispatcher resolves by leaf, but leaves are not unique once scripts may
    // live in any folder.
    let mut leaf_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for src in &sources {
        if let Some(leaf) = src.file_name().and_then(|n| n.to_str()) {
            *leaf_counts.entry(leaf.to_string()).or_default() += 1;
        }
    }

    let mut index = String::new();
    let mut staged = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let leaf = src.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let rel =
            src.strip_prefix(project_dir).unwrap_or(src).to_string_lossy().replace('\\', "/");

        // The editor writes a NEW file per rebuild (a mapped library cannot be
        // overwritten on Windows), so the directory accumulates generations and
        // the newest is the current one.
        let build_dir = project_dir.join(".renzora").join("scripts").join(stem);
        let newest = std::fs::read_dir(&build_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(lib_ext))
            .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());

        let Some(lib) = newest else {
            missing.push(rel);
            continue;
        };

        // Named by index, not by stem: two scripts in different folders may share
        // a stem, and one would overwrite the other.
        let file = format!("script_{i}.{lib_ext}");
        std::fs::copy(&lib, dest.join(&file))
            .map_err(|e| format!("copy {} → {}: {e}", lib.display(), file))?;
        index.push_str(&format!("{rel}\t{file}\n"));
        if leaf_counts.get(leaf).copied().unwrap_or(0) == 1 {
            index.push_str(&format!("{leaf}\t{file}\n"));
        }
        staged += 1;
    }

    if !missing.is_empty() {
        progress(format!(
            "WARN: {} script(s) have no compiled library and were not shipped ({}). \
             Open the project in the editor so they build, then export again.",
            missing.len(),
            missing.join(", ")
        ));
    }
    if staged == 0 {
        let _ = std::fs::remove_dir_all(&dest);
        return Ok(0);
    }
    std::fs::write(dest.join(renzora_rust_script::PREBUILT_MANIFEST), index)
        .map_err(|e| format!("write script index: {e}"))?;
    progress(format!("Shipped {staged} compiled Rust script(s)"));
    Ok(staged)
}

/// The project's scripts, as the EDITOR defines them.
///
/// Delegated rather than reimplemented. The two had their own scans for a while
/// and disagreed — the editor read `<project>/scripts/` flat, this walked the
/// whole tree — so a script in a subfolder was compiled into a lean export
/// having never run in the editor once. Sharing the definition means an export
/// can only ever ship what the editor would have built.
fn collect_scripts(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    out.extend(renzora_rust_script::collect_project_scripts(dir));
    Ok(())
}

/// Whether a script's source asks `renzora::script!` for a hook entry point.
///
/// Text, not syntax, for the same reason the `add!` plugin generator reads text:
/// `script!` is a macro, so at staging time there is nothing but the call. The
/// match is deliberately narrow — the `hooks =` must sit inside a `script!(…)`
/// argument list — so a script that merely has a function called `hooks`, or the
/// word in a comment, does not get a table row pointing at a symbol its macro
/// never generated. That would be a link error rather than a silent problem, but
/// a link error in generated code nobody wrote is a bad way to find out.
fn declares_hooks(source: &str) -> bool {
    source.split("script!").skip(1).any(|rest| {
        rest.split_once(')')
            .is_some_and(|(args, _)| args.contains("hooks") && args.contains('='))
    })
}

fn stage_static_scripts(
    project_dir: &Path,
    copy_root: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<bool, String> {
    let crate_dir = copy_root.join("crates").join("renzora_static_scripts");

    // The WHOLE project, recursively — not just `scripts/`. A script is attached
    // by path, and nothing requires that path to live in one directory: a project
    // may keep them beside the scenes that use them, or grouped under
    // `scripts/enemies/`. Scanning one folder would omit the rest and the export
    // would build cleanly, ship, and report `No backend` for exactly the scripts
    // that were somewhere else.
    let mut sources: Vec<PathBuf> = Vec::new();
    collect_scripts(project_dir, &mut sources)?;
    // Sorted so a rebuild with no source change produces byte-identical output
    // and `write_if_changed` keeps cargo from recompiling the crate.
    sources.sort();

    if sources.is_empty() {
        return Ok(false);
    }

    std::fs::create_dir_all(crate_dir.join("src"))
        .map_err(|e| format!("create {}: {e}", crate_dir.display()))?;

    // Copied beside the generated lib rather than referenced where they live:
    // a `#[path]` pointing outside the workspace works, but then the export's
    // build depends on the project directory staying put mid-compile, and the
    // throwaway copy stops being self-contained.
    // Leaf names are no longer unique now the scan is recursive — a project may
    // hold `enemies/spin.rs` and `props/spin.rs`. Count them so an ambiguous leaf
    // can be left out of the table rather than silently resolving to whichever
    // was written last.
    let mut leaf_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for src in &sources {
        if let Some(leaf) = src.file_name().and_then(|n| n.to_str()) {
            *leaf_counts.entry(leaf.to_string()).or_default() += 1;
        }
    }

    let mut mods = String::new();
    let mut entries = String::new();
    let mut ambiguous: Vec<String> = Vec::new();
    // Does any script name the UI crate? The SDK a script is normally compiled
    // against hands it three crates — `bevy`, `renzora` and `renzora_ember` —
    // so a script reading a HUD widget or a markup name index is perfectly
    // legal, and used to fail this build with "unlinked crate renzora_ember"
    // because the generated manifest below only ever listed two of the three.
    // Added on demand rather than always: a game with no UI strips ember
    // entirely, and an unconditional dep would compile it back in.
    let mut uses_ember = false;
    // The scripts that also exported a lifecycle hook, as table rows.
    //
    // `renzora::script!(update, hooks = …)` emits a second entry point beside
    // `update`. The dylib path finds it by symbol lookup, which a static build
    // has no equivalent of — so before this table a script's `on_scene_loaded`
    // fired in the editor and silently never fired in a lean export. A loading
    // screen is exactly that hook, so the difference was not academic.
    let mut hook_entries = String::new();
    for (i, src) in sources.iter().enumerate() {
        let leaf = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("unreadable script name: {}", src.display()))?;
        // Project-relative, forward-slashed: the spelling a `ScriptComponent`
        // holds, and unique by construction where a leaf is not.
        let rel = src
            .strip_prefix(project_dir)
            .unwrap_or(src)
            .to_string_lossy()
            .replace('\\', "/");

        // Staged under an index, not its leaf, or two `spin.rs` would overwrite
        // each other in `src/` and one script would silently become the other.
        let staged_name = format!("script_{i}.rs");
        let text = std::fs::read_to_string(src)
            .map_err(|e| format!("read {}: {e}", src.display()))?;
        uses_ember |= text.contains("renzora_ember");
        write_if_changed(&crate_dir.join("src").join(&staged_name), &text)?;

        mods.push_str(&format!("#[path = \"{staged_name}\"]\nmod script_{i};\n"));
        // Registered under the relative path always…
        entries.push_str(&format!(
            "        (\"{rel}\", script_{i}::renzora_script_update as ScriptFn),\n"
        ));
        // …and under the bare leaf too when that is unambiguous, because the
        // dispatcher resolves by leaf: a `ScriptComponent` entry may hold either
        // spelling depending on how it was added. Registering both keys means
        // neither lookup has to change.
        // The hook table, under the same keys, for the scripts that have one.
        // Read from the source rather than from a symbol: `script!` is a macro,
        // so the only thing that exists at this point is the text that will
        // expand into `renzora_script_hook`.
        if declares_hooks(&text) {
            hook_entries.push_str(&format!(
                "        (\"{rel}\", script_{i}::renzora_script_hook as HookFn),\n"
            ));
            if leaf_counts.get(leaf).copied().unwrap_or(0) == 1 {
                hook_entries.push_str(&format!(
                    "        (\"{leaf}\", script_{i}::renzora_script_hook as HookFn),\n"
                ));
            }
        }
        if leaf_counts.get(leaf).copied().unwrap_or(0) == 1 {
            entries.push_str(&format!(
                "        (\"{leaf}\", script_{i}::renzora_script_update as ScriptFn),\n"
            ));
        } else if !ambiguous.contains(&leaf.to_string()) {
            ambiguous.push(leaf.to_string());
        }
    }

    // Named rather than silently dropped: a script attached by bare leaf that
    // now resolves to nothing would otherwise look like the export lost it.
    if !ambiguous.is_empty() {
        progress(format!(
            "Note: {} script name(s) appear in more than one folder ({}). Those are \
             reachable only by their full project-relative path — attaching one by \
             file name alone is ambiguous and will not resolve.",
            ambiguous.len(),
            ambiguous.join(", ")
        ));
    }

    // A plain literal — nothing is substituted. The braces below are single now:
    // they were doubled to escape them for `format!`, and reading them literally
    // would have emitted `renzora = {{ path = … }}` into the manifest.
    let mut manifest = String::from(
        "# GENERATED by the lean exporter — see `renzora_export::build`.\n\
         # The project's Rust scripts, compiled into this build's binary.\n\
         [package]\n\
         name = \"renzora_static_scripts\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\
         \n\
         [dependencies]\n\
         # `static_scripts` drops `#[no_mangle]` from what `renzora::script!`\n\
         # emits, so the scripts below can share one binary. Cargo unifies\n\
         # features per package, so naming it once here applies it to all.\n\
         renzora = { path = \"../renzora\", features = [\"static_scripts\"] }\n\
         bevy = { workspace = true }\n"
    );
    if uses_ember {
        // `default-features = false` for the same reason `renzora_runtime` deps
        // it that way: which of ember's features are on is the export's decision,
        // not this crate's, and turning them on here would quietly un-strip
        // `game_ui` (and the world-space 3D UI) in a build that dropped them.
        manifest.push_str(
            "# At least one script imports the UI crate.\n\
             renzora_ember = { path = \"../renzora_ember\", default-features = false }\n",
        );
    }
    manifest.push_str(
        "\n\
         [lints]\n\
         workspace = true\n",
    );
    write_if_changed(&crate_dir.join("Cargo.toml"), &manifest)?;

    let lib = format!(
        "//! GENERATED by the lean exporter — see `renzora_export::build`.\n\
         //!\n\
         //! The project's Rust scripts, compiled into this binary. Overwritten on\n\
         //! every lean export; edits here do not survive one.\n\
         \n\
         use bevy::ecs::entity::Entity;\n\
         use bevy::ecs::world::World;\n\
         \n\
         pub type ScriptFn = fn(&mut World, Entity);\n\
         pub type HookFn = fn(&mut World, Entity, &renzora::ScriptHook<'_>);\n\
         \n\
         {mods}\n\
         /// Every script compiled in, as `(file name, entry point)`.\n\
         pub fn scripts() -> Vec<(&'static str, ScriptFn)> {{\n\
         \x20   vec![\n{entries}    ]\n\
         }}\n\
         \n\
         /// Every script compiled in that also exported a lifecycle hook.\n\
         pub fn hooks() -> Vec<(&'static str, HookFn)> {{\n\
         \x20   vec![\n{hook_entries}    ]\n\
         }}\n"
    );
    write_if_changed(&crate_dir.join("src").join("lib.rs"), &lib)?;

    progress(format!(
        "Compiling {} Rust script(s) into the binary: {}",
        sources.len(),
        sources
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    Ok(true)
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == content) {
        return Ok(());
    }
    std::fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Copy-if-newer of `src` → `dest`, skipping build/vcs noise and one named file
/// (the manifest, which [`stage_static_plugins`] writes patched instead).
///
/// `Cargo.lock` rides along harmlessly: a path dependency's lockfile is ignored,
/// the workspace's own is what resolves the build.
fn sync_dir_except(
    src: &Path,
    dest: &Path,
    skip_file: &str,
    copied: &mut usize,
) -> Result<(), String> {
    const DEEP_SKIP: &[&str] = &["target", ".git"];
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let s = entry.path();
        let d = dest.join(&name);
        if ft.is_dir() {
            if DEEP_SKIP.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            std::fs::create_dir_all(&d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
            sync_dir(&s, &d, copied)?;
        } else if ft.is_file() && name.to_string_lossy() != skip_file && should_copy(&s, &d) {
            std::fs::copy(&s, &d).map_err(|e| format!("copy {}: {e}", s.display()))?;
            *copied += 1;
        }
    }
    Ok(())
}

/// Read the plugin's real manifest, apply the three edits described on
/// [`stage_static_plugins`], and write the result into the copy.
///
/// Reads the pristine source and writes only on a difference, so an unchanged
/// plugin selection leaves the copied manifest's mtime alone — patching in place
/// over a synced file could not do that, because the patched and pristine
/// versions differ in size and the sync would keep clobbering it.
fn patch_plugin_manifest(src_dir: &Path, dest_manifest: &Path) -> Result<(), String> {
    let src = src_dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&src)
        .map_err(|e| format!("read {}: {e}", src.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", src.display()))?;

    doc.remove("workspace");
    doc.remove("profile");

    // Repoint the engine dependencies at THIS workspace's copies.
    //
    // A plugin's manifest names them by relative path, and the path that is
    // correct where the plugin is installed is wrong here. An installed
    // standalone plugin says `../../sdk/plugin-api/renzora_plugin` — written by
    // `renzora_native_plugin::standalone::repoint_contract`, and correct from
    // `<editor>/plugins/<id>/`. Copied to `<workspace>/plugins/<id>/` the same
    // string resolves to a `sdk/` directory the export workspace has never had,
    // and cargo refuses the whole workspace before compiling a line:
    //
    //   failed to read .../export-src/sdk/plugin-api/renzora_plugin/Cargo.toml
    //
    // Rewriting rather than assuming is what makes this hold for a plugin from
    // anywhere. A marketplace download, a plugin developed in its author's own
    // checkout and one repointed by the editor all name that crate differently,
    // and all three are correct where they came from.
    //
    // Only `path` dependencies are touched: a plugin depending on a published
    // `renzora_plugin` from crates.io is naming a version, and cargo resolves it
    // the same way here as anywhere.
    if let Some(deps) = doc.get_mut("dependencies").and_then(|d| d.as_table_like_mut()) {
        for (name, workspace_path) in [
            ("renzora_plugin", "../../crates/renzora_plugin"),
            // The native half. `renzora` and `bevy` are what a native plugin
            // links, and `renzora` is the one named by path.
            ("renzora", "../../crates/renzora"),
            ("renzora_ember", "../../crates/renzora_ember"),
        ] {
            let Some(dep) = deps.get_mut(name).and_then(|d| d.as_table_like_mut()) else {
                continue;
            };
            if dep.get("path").is_some() {
                dep.insert("path", toml_edit::value(workspace_path));
            }
        }
    }
    let lib = doc
        .entry("lib")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    if let Some(lib) = lib.as_table_like_mut() {
        let mut kinds = toml_edit::Array::new();
        kinds.push("rlib");
        lib.insert("crate-type", toml_edit::value(kinds));
    }

    write_if_changed(dest_manifest, &doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The minimum that counts as a script: it calls `renzora::script!`, which
    /// is what `collect_project_scripts` looks for.
    const SCRIPT: &str = "fn update(_c: &mut ScriptCtx) {}\nrenzora::script!(update);\n";

    /// A project with two scripts produces a module and a table entry for each,
    /// and copies the sources beside the generated lib.
    ///
    /// The module-per-script is what keeps them from colliding: both may define
    /// a `renzora_script_update` (the `no_mangle` is dropped under
    /// `static_scripts`) and both may define their own `Spin` component.
    #[test]
    fn static_scripts_generate_a_module_and_entry_per_file() {
        let project = std::env::temp_dir()
            .join(format!("renzora_static_scripts_{}", std::process::id()));
        let scripts = project.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        // Written out of order to prove the sort: `orbit` must come first, so a
        // rebuild with no source change regenerates byte-identical output.
        std::fs::write(scripts.join("spin.rs"), SCRIPT).unwrap();
        std::fs::write(scripts.join("orbit.rs"), SCRIPT).unwrap();
        // Not a script — must not be compiled in.
        std::fs::write(scripts.join("notes.txt"), "ignore me").unwrap();
        // Rust, but not a script: no `script!`, so nothing to call. Compiling it
        // would fail on a file its author never called a script.
        std::fs::write(scripts.join("helper.rs"), "pub fn helper() {}").unwrap();

        let ws = project.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        let staged = stage_static_scripts(&project, &ws, &mut |_| {}).unwrap();
        assert!(staged, "two scripts is something to link");

        let crate_dir = ws.join("crates").join("renzora_static_scripts");
        let lib = std::fs::read_to_string(crate_dir.join("src").join("lib.rs")).unwrap();
        assert!(lib.contains("#[path = \"script_0.rs\"]\nmod script_0;"), "{lib}");
        assert!(lib.contains("#[path = \"script_1.rs\"]\nmod script_1;"), "{lib}");
        // Both spellings: the relative path, and the leaf while it is unique.
        assert!(lib.contains("(\"scripts/orbit.rs\", script_0::"), "{lib}");
        assert!(lib.contains("(\"orbit.rs\", script_0::"), "{lib}");
        assert!(lib.contains("(\"scripts/spin.rs\", script_1::"), "{lib}");
        assert!(!lib.contains("notes"), "non-scripts must not be linked in: {lib}");

        // Sources copied beside the generated lib under their index, so the
        // export copy stays self-contained and two same-named scripts in
        // different folders cannot overwrite each other.
        assert_eq!(
            std::fs::read_to_string(crate_dir.join("src").join("script_1.rs")).unwrap(),
            SCRIPT
        );
        // Rust that is not a script stays out — a lean export compiles every
        // collected file into the binary, so one non-script would fail the whole
        // build rather than be skipped.
        assert!(!lib.contains("helper"), "non-scripts must not be linked in: {lib}");

        // The manifest must turn the feature on, or every script keeps its
        // `#[no_mangle]` and the binary fails to link.
        let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("features = [\"static_scripts\"]"), "{manifest}");
        // No script here mentions the UI crate, so it must not be linked: a game
        // with no UI strips ember, and an unconditional dep would compile it back.
        assert!(!manifest.contains("renzora_ember"), "{manifest}");

        let _ = std::fs::remove_dir_all(&project);
    }

    /// The stripper must trim EVERY bevy feature list in the root manifest, not
    /// just the workspace one.
    ///
    /// The root manifest carries a second, target-gated list — the features the
    /// web build cannot have. Trimming only the first made three capabilities
    /// no-ops on every desktop export: "Editor/dev conveniences" removed
    /// `file_watcher`, `system_clipboard` and `clipboard_image` from the
    /// workspace list and the target block put all three back, taking
    /// `bevy_clipboard`, `arboard` and a second `image` decoder stack into games
    /// that had explicitly asked for none of it.
    #[test]
    fn every_bevy_feature_list_in_the_root_manifest_is_stripped() {
        let dir = std::env::temp_dir().join(format!("renzora_strip_bevy_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // The real manifest's shape: a workspace list, a desktop-only block that
        // ADDS to it, a web-only block that does the same for the web, and a
        // block with dependencies but NO bevy entry. The last one is the trap —
        // walking it mutably vivifies `bevy = {}` and cargo then refuses the
        // whole file.
        std::fs::write(
            dir.join("Cargo.toml"),
            "[workspace.dependencies]\n\
             bevy = { version = \"0.19\", features = [\"bevy_ui\", \"file_watcher\", \"bevy_pbr\"] }\n\
             \n\
             [target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\n\
             bevy = { workspace = true, features = [\"file_watcher\", \"system_clipboard\"] }\n\
             \n\
             [target.'cfg(target_arch = \"wasm32\")'.dependencies]\n\
             bevy = { workspace = true, features = [\"webgpu\", \"system_clipboard\"] }\n\
             wasm-bindgen = \"0.2\"\n\
             \n\
             [target.'cfg(unix)'.dependencies]\n\
             libc = \"0.2\"\n",
        )
        .unwrap();

        strip_bevy_features(
            &dir,
            &["file_watcher".into(), "system_clipboard".into()],
            &mut |_| {},
        )
        .unwrap();

        let out = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(!out.contains("file_watcher"), "{out}");
        // Twice over: the desktop block AND the web one had it.
        assert!(!out.contains("system_clipboard"), "{out}");
        // Untouched features survive in every list.
        assert!(out.contains("bevy_ui") && out.contains("bevy_pbr"), "{out}");
        assert!(out.contains("webgpu"), "the web's own entry was lost: {out}");
        // And the bevy-less target block is untouched — no invented entry.
        assert!(!out.contains("bevy = {}"), "vivified a bevy key: {out}");
        assert!(out.contains("wasm-bindgen") && out.contains("libc"), "{out}");
        // The strongest check: cargo must still be able to parse what we wrote.
        assert!(
            out.parse::<toml_edit::DocumentMut>().is_ok(),
            "result is not valid TOML: {out}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A script's lifecycle hook reaches the binary too.
    ///
    /// The generated table used to be `(name, update_fn)` only, so a lean export
    /// ran `update` and delivered no events — a loading screen's
    /// `on_scene_loaded` fired in the editor and never fired in the shipped
    /// game. Only scripts that actually asked for a hook get a row: the symbol
    /// the row names does not exist otherwise.
    #[test]
    fn a_script_with_lifecycle_hooks_gets_a_hook_table_row() {
        let project =
            std::env::temp_dir().join(format!("renzora_static_hooks_{}", std::process::id()));
        let scripts = project.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("curtain.rs"),
            "fn update(_c: &mut ScriptCtx) {}\n\
             fn hooks(_c: &mut ScriptCtx, _h: &ScriptHook) {}\n\
             renzora::script!(update, hooks = hooks);\n",
        )
        .unwrap();
        std::fs::write(scripts.join("plain.rs"), SCRIPT).unwrap();

        let ws = project.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(stage_static_scripts(&project, &ws, &mut |_| {}).unwrap());

        let lib = std::fs::read_to_string(
            ws.join("crates")
                .join("renzora_static_scripts")
                .join("src")
                .join("lib.rs"),
        )
        .unwrap();
        assert!(lib.contains("pub fn hooks() -> Vec<(&'static str, HookFn)>"), "{lib}");
        assert!(lib.contains("(\"curtain.rs\", script_0::renzora_script_hook"), "{lib}");
        // `plain.rs` never asked for one, so naming its symbol would not link.
        assert!(!lib.contains("script_1::renzora_script_hook"), "{lib}");

        let _ = std::fs::remove_dir_all(&project);
    }

    /// Only a `hooks =` inside a `script!` argument list counts.
    #[test]
    fn hook_detection_ignores_lookalikes() {
        assert!(declares_hooks("renzora::script!(update, hooks = my_hooks);"));
        assert!(declares_hooks("script!( update , hooks=h );"));
        // A function named `hooks`, or the word in prose, is not a declaration.
        assert!(!declares_hooks("fn hooks() {}\nrenzora::script!(update);"));
        assert!(!declares_hooks("// hooks = something\nrenzora::script!(update);"));
        assert!(!declares_hooks(SCRIPT));
    }

    /// A script that imports `renzora_ember` gets the dependency it needs.
    ///
    /// The SDK a script is normally compiled against hands it three crates —
    /// `bevy`, `renzora` and `renzora_ember` — so reading a HUD widget from a
    /// script is ordinary. The generated manifest listed only two of the three,
    /// and such a script failed the lean build on "unlinked crate renzora_ember"
    /// after having run fine in the editor for as long as the author had been
    /// writing it.
    /// Build a `plugins/` tree: one directory per entry, with a manifest and a
    /// `src/lib.rs` so both kinds are readable.
    fn plugin_tree(tag: &str, entries: &[(&str, &str, &str)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("renzora_plan_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (name, manifest, lib) in entries {
            let dir = root.join("plugins").join(name);
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
            std::fs::write(dir.join("src").join("lib.rs"), lib).unwrap();
        }
        root
    }

    fn cabi(name: &str, unsupported: &str) -> String {
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n{unsupported}\n\
             [lib]\ncrate-type = [\"cdylib\"]\n"
        )
    }

    /// The exporter must not try to link a plugin that cannot build for the
    /// target.
    ///
    /// Fixtures rather than the repository's own plugins, which this used to
    /// read: they no longer live here — they are distributed through the
    /// marketplace and installed beside the editor — so a test pinned to their
    /// manifests was asserting on a directory that no longer exists. What is
    /// left is the rule itself, which is what the exporter actually implements.
    #[test]
    fn a_plugin_that_cannot_cross_to_the_target_is_not_linked() {
        let gate = "[package.metadata.renzora]\nunsupported-targets = [\"wasm32\"]\n";
        let repo = plugin_tree(
            "web",
            &[
                ("audio", &cabi("audio", gate), ""),
                ("grayscale", &cabi("grayscale", ""), ""),
            ],
        );
        let wanted: Vec<(String, bool)> =
            ["audio", "grayscale"].iter().map(|id| (id.to_string(), false)).collect();

        let web = resolve_static_plugins(&repo.join("plugins"), &wanted, Some("wasm32-unknown-unknown"));
        assert!(
            web.linked.iter().all(|p| p.id == "grayscale"),
            "only the ungated one should cross: {:?}",
            web.linked.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
        assert_eq!(web.unsupported, ["audio"]);
        assert!(web.no_source.is_empty(), "{:?}", web.no_source);

        // The same set on a desktop triple links both: the key is per-target,
        // not a blanket exclusion.
        let desktop = resolve_static_plugins(&repo.join("plugins"), &wanted, Some("x86_64-pc-windows-msvc"));
        assert_eq!(desktop.linked.len(), 2, "{:?}", desktop.unsupported);
        assert!(desktop.unsupported.is_empty());
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// No `--target` means the host, where nothing is filtered.
    #[test]
    fn no_target_filters_nothing() {
        let gate = "[package.metadata.renzora]\nunsupported-targets = [\"wasm32\"]\n";
        let repo = plugin_tree("hosttriple", &[("audio", &cabi("audio", gate), "")]);
        let plan = resolve_static_plugins(&repo.join("plugins"), &[("audio".to_string(), false)], None);
        assert_eq!(plan.linked.len(), 1);
        assert!(plan.unsupported.is_empty());
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// A native plugin is linkable too, and is described by CODE rather than by
    /// a table: the plan has to carry the expression `plugin!` was given, since
    /// installing one is `add_plugins(<crate>::<expr>)` and there is no symbol
    /// to look up.
    #[test]
    fn a_native_plugin_is_planned_with_its_type() {
        let repo = plugin_tree(
            "native",
            &[
                (
                    "spline",
                    "[package]\nname = \"spline\"\nversion = \"0.1.0\"\n\
                     [lib]\ncrate-type = [\"dylib\"]\n",
                    "renzora::plugin!(SplinePlugin, Runtime);\n",
                ),
                ("grayscale", &cabi("grayscale", ""), ""),
            ],
        );
        let wanted: Vec<(String, bool)> =
            ["spline", "grayscale"].iter().map(|id| (id.to_string(), false)).collect();
        let plan = resolve_static_plugins(&repo.join("plugins"), &wanted, None);

        let spline = plan.linked.iter().find(|p| p.id == "spline").expect("spline linked");
        assert_eq!(spline.kind, StaticKind::Native { expr: "SplinePlugin".into() });
        let grayscale = plan.linked.iter().find(|p| p.id == "grayscale").expect("grayscale");
        assert_eq!(grayscale.kind, StaticKind::CAbi);
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// A manifest with no `[package.metadata.renzora]` — nearly all of them —
    /// must read as "builds anywhere", and so must an unparseable one.
    #[test]
    fn an_absent_or_broken_key_means_buildable() {
        assert!(supports_target("[package]\nname = \"x\"\n", "wasm32-unknown-unknown"));
        assert!(supports_target("this is not toml {{{", "wasm32-unknown-unknown"));
        assert!(supports_target(
            "[package.metadata.renzora]\nunsupported-targets = [\"wasm32\"]\n",
            "x86_64-pc-windows-msvc"
        ));
        assert!(!supports_target(
            "[package.metadata.renzora]\nunsupported-targets = [\"apple-darwin\"]\n",
            "aarch64-apple-darwin"
        ));
    }

    #[test]
    fn a_script_that_uses_the_ui_crate_gets_it_linked() {
        let project =
            std::env::temp_dir().join(format!("renzora_static_ember_{}", std::process::id()));
        let scripts = project.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("hud.rs"),
            format!("use renzora_ember::game_ui::components::UiImageFill;\n{SCRIPT}"),
        )
        .unwrap();

        let ws = project.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(stage_static_scripts(&project, &ws, &mut |_| {}).unwrap());

        let manifest = std::fs::read_to_string(
            ws.join("crates")
                .join("renzora_static_scripts")
                .join("Cargo.toml"),
        )
        .unwrap();
        assert!(manifest.contains("renzora_ember"), "{manifest}");
        // Which of ember's features are on is the export's decision, not this
        // crate's — turning them on here would un-strip `game_ui` and the
        // world-space 3D UI in a build that dropped them.
        assert!(manifest.contains("default-features = false"), "{manifest}");

        let _ = std::fs::remove_dir_all(&project);
    }

    /// Scripts are found anywhere in the project, build output is skipped, and a
    /// name used in two folders stays reachable by path without either script
    /// silently becoming the other.
    #[test]
    fn static_scripts_scan_the_whole_project() {
        let project = std::env::temp_dir()
            .join(format!("renzora_scan_project_{}", std::process::id()));
        for sub in ["scripts", "enemies", "props", ".renzora/scripts", "target"] {
            std::fs::create_dir_all(project.join(sub)).unwrap();
        }
        std::fs::write(project.join("scripts/orbit.rs"), SCRIPT).unwrap();
        // Same leaf in two different folders — legal, and ambiguous by leaf.
        std::fs::write(project.join("enemies/spin.rs"), SCRIPT).unwrap();
        std::fs::write(project.join("props/spin.rs"), SCRIPT).unwrap();
        // Build output must never be compiled in: `.renzora/` holds the editor's
        // own staged copies, and `target/` is cargo's.
        std::fs::write(project.join(".renzora/scripts/orbit.rs"), SCRIPT).unwrap();
        std::fs::write(project.join("target/leftover.rs"), SCRIPT).unwrap();

        let ws = project.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(stage_static_scripts(&project, &ws, &mut |_| {}).unwrap());

        let lib = std::fs::read_to_string(
            ws.join("crates/renzora_static_scripts/src/lib.rs"),
        )
        .unwrap();

        // Found outside `scripts/`.
        assert!(lib.contains("(\"enemies/spin.rs\""), "{lib}");
        assert!(lib.contains("(\"props/spin.rs\""), "{lib}");
        // Ambiguous leaf registered under neither bare name.
        assert!(!lib.contains("(\"spin.rs\""), "ambiguous leaf must not resolve: {lib}");
        // Unique leaf still gets its shorthand.
        assert!(lib.contains("(\"orbit.rs\""), "{lib}");
        // Build output excluded.
        assert!(!lib.contains("leftover"), "{lib}");
        assert!(!lib.contains(".renzora"), "{lib}");

        let _ = std::fs::remove_dir_all(&project);
    }

    /// A project with no scripts generates nothing, so the caller leaves the
    /// feature off rather than compiling an empty aggregator.
    #[test]
    fn no_scripts_generates_nothing() {
        let project = std::env::temp_dir()
            .join(format!("renzora_no_scripts_{}", std::process::id()));
        std::fs::create_dir_all(project.join("scripts")).unwrap();
        let ws = project.join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        assert!(!stage_static_scripts(&project, &ws, &mut |_| {}).unwrap());
        assert!(!ws.join("crates").join("renzora_static_scripts").exists());

        let _ = std::fs::remove_dir_all(&project);
    }

    /// The two lines of `[profile.dist-lean]` the patcher edits, as they appear
    /// in the real root manifest.
    const MANIFEST: &str = "\
[profile.dist-lean]
inherits = \"dist\"
lto = \"thin\"
opt-level = \"s\"
strip = \"symbols\"
";

    fn patch(opts: LeanProfile) -> String {
        // Unique per test so a parallel run can't share a manifest. No tempfile
        // dev-dependency in this crate, and one directory per (pid, opts) is
        // enough — the values differ in every call site below.
        let dir = std::env::temp_dir().join(format!(
            "renzora_lean_profile_{}_{}{}{}",
            std::process::id(),
            opts.panic_abort as u8,
            opts.opt_level_z as u8,
            opts.codegen_units_one as u8,
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), MANIFEST).unwrap();
        patch_lean_profile(&dir, opts, &mut |_| {}).unwrap();
        let out = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn all_off_leaves_the_profile_at_its_checked_in_settings() {
        let out = patch(LeanProfile::default());
        assert!(out.contains("opt-level = \"s\""), "{out}");
        assert!(!out.contains("panic"), "{out}");
        assert!(!out.contains("codegen-units"), "{out}");
        // The knobs must not disturb what the profile already says.
        assert!(out.contains("lto = \"thin\""), "{out}");
        assert!(out.contains("strip = \"symbols\""), "{out}");
    }

    #[test]
    fn all_on_writes_every_knob() {
        let out = patch(LeanProfile {
            panic_abort: true,
            opt_level_z: true,
            codegen_units_one: true,
        });
        assert!(out.contains("panic = \"abort\""), "{out}");
        assert!(out.contains("opt-level = \"z\""), "{out}");
        assert!(out.contains("codegen-units = 1"), "{out}");
    }

    /// The export copy persists between exports, so turning a knob back off has
    /// to actually remove what the last export wrote — otherwise a stale
    /// `panic = "abort"` silently applies to a build that asked for unwinding.
    #[test]
    fn turning_a_knob_back_off_reverts_the_manifest() {
        let dir = std::env::temp_dir()
            .join(format!("renzora_lean_profile_revert_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), MANIFEST).unwrap();

        let on = LeanProfile {
            panic_abort: true,
            opt_level_z: true,
            codegen_units_one: true,
        };
        patch_lean_profile(&dir, on, &mut |_| {}).unwrap();
        patch_lean_profile(&dir, LeanProfile::default(), &mut |_| {}).unwrap();

        let out = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!out.contains("panic"), "{out}");
        assert!(!out.contains("codegen-units"), "{out}");
        // `opt-level` is set rather than removed: removing it would inherit
        // `dist`'s speed-tuned `opt-level = 2`.
        assert!(out.contains("opt-level = \"s\""), "{out}");
    }
}

#[cfg(test)]
mod native_static_tests {
    use super::{declares_dylib, native_plugin_expr};

    #[test]
    fn crate_type_tells_the_two_kinds_apart() {
        assert!(declares_dylib("[lib]\ncrate-type = [\"dylib\"]\n"));
        // `"cdylib"` ends in `dylib`; a loose match would call every C-ABI
        // plugin native and generate an `add_plugins` call for a type that
        // does not exist.
        assert!(!declares_dylib("[lib]\ncrate-type = [\"cdylib\"]\n"));
        assert!(!declares_dylib("[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n"));
    }

    fn plugin_dir(tag: &str, body: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("renzora_expr_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("src").join("lib.rs"), body).unwrap();
        d
    }

    /// The shape every native plugin in the repository uses.
    #[test]
    fn a_bare_identifier_is_linkable() {
        let d = plugin_dir("bare", "use bevy::prelude::*;\nrenzora::plugin!(SplinePlugin, Runtime);\n");
        assert_eq!(native_plugin_expr(&d).as_deref(), Some("SplinePlugin"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// `plugin!` takes an EXPRESSION, so these are legal and cannot be pasted
    /// after a `::`. Refusing them here ships the plugin as a file; accepting
    /// them would put a syntax error in generated code the author never wrote.
    #[test]
    fn an_expression_that_is_not_a_path_is_refused() {
        for body in [
            "renzora::plugin!(MyPlugin::default(), Runtime);",
            "renzora::plugin!(MyPlugin { size: 4 }, Runtime);",
            "// renzora::plugin!(Commented, Runtime);",
            "fn main() {}",
        ] {
            let d = plugin_dir("expr", body);
            assert_eq!(native_plugin_expr(&d), None, "accepted {body:?}");
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    /// The single-argument form defaults to `Editor`, and still names a type.
    #[test]
    fn the_scopeless_form_is_read_too() {
        let d = plugin_dir("scopeless", "renzora::plugin!(ToolPlugin);\n");
        assert_eq!(native_plugin_expr(&d).as_deref(), Some("ToolPlugin"));
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod manifest_patch_tests {
    use super::patch_plugin_manifest;

    fn patched(manifest: &str) -> String {
        let src = std::env::temp_dir()
            .join(format!("renzora_patch_{}_{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&src);
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("Cargo.toml"), manifest).unwrap();
        let dest = src.join("out.toml");
        patch_plugin_manifest(&src, &dest).unwrap();
        let out = std::fs::read_to_string(&dest).unwrap();
        let _ = std::fs::remove_dir_all(&src);
        out
    }

    /// What an INSTALLED standalone plugin's manifest says. That path is correct
    /// beside the editor and names nothing inside the export workspace, and
    /// cargo refuses the entire workspace over it before compiling anything.
    #[test]
    fn an_installed_plugins_contract_path_is_repointed() {
        let out = patched(
            "[package]\nname = \"flock\"\nversion = \"0.1.0\"\n\n             [lib]\ncrate-type = [\"cdylib\"]\n\n             [dependencies]\n             renzora_plugin = { path = \"../../sdk/plugin-api/renzora_plugin\",              default-features = false, features = [\"libm\"] }\n",
        );
        assert!(out.contains("../../crates/renzora_plugin"), "{out}");
        assert!(!out.contains("sdk/plugin-api"), "{out}");
        // The features are what make a `no_std` plugin compile; a rewrite that
        // dropped them would trade one failure for a stranger one.
        assert!(out.contains("default-features = false"), "{out}");
        assert!(out.contains("libm"), "{out}");
        // And the crate type has to become an rlib to be linked in.
        assert!(out.contains("rlib"), "{out}");
    }

    /// A native plugin names `renzora` by path for the same reason and needs the
    /// same treatment; `bevy` comes from the registry and unifies on its own.
    #[test]
    fn a_native_plugins_engine_paths_are_repointed() {
        let out = patched(
            "[package]\nname = \"clouds\"\nversion = \"0.1.0\"\n\n             [lib]\ncrate-type = [\"dylib\"]\n\n             [dependencies]\n             bevy = \"0.19\"\n             renzora = { path = \"/somewhere/else/crates/renzora\" }\n",
        );
        assert!(out.contains("../../crates/renzora"), "{out}");
        assert!(!out.contains("/somewhere/else"), "{out}");
        assert!(out.contains("bevy = \"0.19\""), "{out}");
    }

    /// A version dependency is not a path and must be left alone — cargo
    /// resolves it here exactly as it does anywhere.
    #[test]
    fn a_registry_dependency_is_untouched() {
        let out = patched(
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n             [dependencies]\nrenzora_plugin = \"1\"\n",
        );
        assert!(out.contains("renzora_plugin = \"1\""), "{out}");
        assert!(!out.contains("crates/renzora_plugin"), "{out}");
    }
}

