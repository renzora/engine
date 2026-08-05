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

/// The lean binary's filename under `target/dist-lean/`.
fn bin_filename(platform: Platform) -> &'static str {
    match platform {
        Platform::WindowsX64 => "renzora.exe",
        _ => "renzora",
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
/// The plugins a game uses are NOT compiled in: they are C-ABI plugins, which
/// link no Bevy and so `dlopen` into a static host exactly as into a dynamic
/// one. The caller copies them beside the binary. (An earlier design compiled
/// `renzora::add!`-registering distribution plugins into the binary through a
/// generated `renzora_static_plugins` aggregator; those crates are ordinary
/// feature-gated dependencies of `renzora_runtime` now, so the aggregator, its
/// `static_plugins` feature and the wiring pass are all gone.)
///
/// Native cargo can only target the **host** triple; cross-OS builds are a hard
/// Docker requirement (not yet wired here), so this rejects a non-host target
/// rather than producing a wrong artifact.
pub fn build_lean(
    workspace_dir: &Path,
    platform: Platform,
    toolchain: &Toolchain,
    progress: &mut dyn FnMut(String),
    disabled_bevy_features: &[String],
    disabled_runtime_features: &[String],
    panic_abort: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    if Platform::current() != Some(platform) {
        return Err(format!(
            "Cross-platform lean builds require Docker (not yet available). \
             Build {} natively on a {} host, or use the copy-based export.",
            platform.display_name(),
            platform.display_name(),
        ));
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
    strip_bevy_features(&ws, disabled_bevy_features, progress)?;
    strip_runtime_features(&ws, disabled_runtime_features, progress)?;
    set_panic_abort(&ws, panic_abort, progress)?;
    let features = String::from("runtime");

    let mut cmd = toolchain.cargo_command();
    cmd.current_dir(&ws)
        .env("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags(platform))
        .args([
            "build",
            "--profile",
            "dist-lean",
            "--bin",
            "renzora",
            "--no-default-features",
        ])
        .arg("--features")
        .arg(&features);
    if matches!(platform, Platform::LinuxX64) {
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

    let bin = ws.join("target").join("dist-lean").join(bin_filename(platform));
    if !bin.is_file() {
        return Err(format!(
            "Lean build reported success but the binary is missing at {}",
            bin.display()
        ));
    }
    Ok(bin)
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
    const TOP_SKIP: &[&str] = &[
        "target", ".git", ".github", ".vscode", ".idea", "dist", "docs",
        "node_modules", "templates", "disabled", "docker", ".claude", ".devcontainer",
    ];
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
        } else if ft.is_file() && should_copy(&s, &d) {
            std::fs::copy(&s, &d).map_err(|e| format!("copy {}: {e}", s.display()))?;
            copied += 1;
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
/// libraries (no `cdylib` crate-type) and never match. The game's own plugins
/// live in `plugins/`, not here, and ship as files beside the binary.
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
    let arr = doc
        .get_mut("workspace")
        .and_then(|w| w.get_mut("dependencies"))
        .and_then(|d| d.get_mut("bevy"))
        .and_then(|b| b.get_mut("features"))
        .and_then(|f| f.as_array_mut());
    let Some(arr) = arr else {
        // No workspace bevy feature list to trim — nothing to do.
        return Ok(());
    };
    arr.retain(|v| {
        v.as_str()
            .map(|s| !disabled.iter().any(|d| d == s))
            .unwrap_or(true)
    });
    std::fs::write(&manifest, doc.to_string())
        .map_err(|e| format!("write {}: {e}", manifest.display()))?;
    progress(format!("Stripping {} unused Bevy feature(s)", disabled.len()));
    Ok(())
}

/// Patch `panic = "abort"` into the export copy's `[profile.dist-lean]`.
///
/// The largest single size lever there is — measured 60.9 MB → 46.7 MB on a
/// cube-and-light project, because dropping unwinding removes the landing pads
/// and cleanup glue from `.text` and the panic message/location strings from
/// `.rdata`, not merely the `.pdata` unwind tables.
///
/// Only legal because the copy builds `renzora` as `rlib` only (see
/// `sync_export_workspace`); the dev tree's `dylib` would link the precompiled
/// std's `panic_unwind` and refuse to mix strategies.
///
/// Idempotent, and removes the key again when `abort` is false — the copy
/// persists between exports, so a stale `panic = "abort"` would silently apply
/// to a later export that had switched the capability back on.
fn set_panic_abort(
    copy_root: &Path,
    abort: bool,
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
    let had = profile.get("panic").is_some();
    if abort {
        profile.insert("panic", toml_edit::value("abort"));
    } else {
        profile.remove("panic");
    }
    if abort != had {
        std::fs::write(&manifest, doc.to_string())
            .map_err(|e| format!("write {}: {e}", manifest.display()))?;
    }
    if abort {
        progress("Building with panic = abort (no unwinding)".to_string());
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


