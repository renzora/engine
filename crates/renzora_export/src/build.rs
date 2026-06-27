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
/// `static_plugin_crates` are the workspace crate names (e.g. `renzora_lumen`)
/// of distribution plugins the game uses — they're compiled INTO the binary
/// (a static binary can't `dlopen`), via the `renzora_static_plugins` aggregator.
///
/// Native cargo can only target the **host** triple; cross-OS builds are a hard
/// Docker requirement (not yet wired here), so this rejects a non-host target
/// rather than producing a wrong artifact.
pub fn build_lean(
    workspace_dir: &Path,
    platform: Platform,
    toolchain: &Toolchain,
    progress: &mut dyn FnMut(String),
    static_plugin_crates: &[String],
    disabled_bevy_features: &[String],
    disabled_runtime_features: &[String],
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
    // patched freely with no restore (it's disposable): `renzora` → rlib-only (its
    // dylib would blow the PE 65535-export cap), plus any selected plugins wired
    // into the static aggregator. The copy has its own `target/`, so the dev cache
    // and locks are untouched and exports stay incremental across runs.
    let ws = sync_export_workspace(workspace_dir, static_plugin_crates, progress)?;
    strip_bevy_features(&ws, disabled_bevy_features, progress)?;
    strip_runtime_features(&ws, disabled_runtime_features, progress)?;
    force_rlib_only(&ws.join("crates").join("renzora").join("Cargo.toml"))?;
    let has_static_plugins =
        wire_static_plugins(&ws.join("crates"), static_plugin_crates, progress)?;
    let mut features = String::from("runtime");
    if has_static_plugins {
        features.push_str(",static_plugins");
    }

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
    selected_plugins: &[String],
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
    // cdylib distribution plugins the game doesn't use are never linked into the
    // lean binary, so leave them out of the copy entirely.
    let drop_plugins = unselected_cdylib_plugins(engine_src, selected_plugins);
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

/// Sync `crates/`, but skip (and prune from the copy) the unselected cdylib
/// distribution plugins — they're never linked into the lean binary, so copying
/// and resolving them is pure waste. Everything else (core rlib crates, vendored
/// crates, selected plugins, the aggregator) syncs normally.
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

/// Names of `crates/` entries that are cdylib distribution plugins (dlopen-only,
/// NOT in the lean binary's link closure) and were NOT selected for this export.
/// Core runtime subsystems are rlib libraries (no `cdylib` crate-type) and never
/// match; selected plugins (which the aggregator links) are kept.
fn unselected_cdylib_plugins(engine_src: &Path, selected: &[String]) -> HashSet<String> {
    let mut drop = HashSet::new();
    let Ok(rd) = std::fs::read_dir(engine_src.join("crates")) else {
        return drop;
    };
    for entry in rd.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if selected.contains(&name) {
            continue;
        }
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

/// Rewrite a crate's `[lib] crate-type` to exactly `["rlib"]`. Applied to the
/// disposable export copy (no restore needed). Used to drop `renzora`'s `dylib`
/// artifact: in a static build it re-exports all of bevy and would blow the
/// Windows PE 65535-export cap. Robust to whatever's inside the brackets.
fn force_rlib_only(manifest: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(manifest).map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let key = text
        .find("crate-type")
        .ok_or_else(|| format!("no crate-type in {}", manifest.display()))?;
    let lb = text[key..]
        .find('[')
        .map(|i| key + i)
        .ok_or_else(|| format!("malformed crate-type in {}", manifest.display()))?;
    let rb = text[lb..]
        .find(']')
        .map(|i| lb + i)
        .ok_or_else(|| format!("malformed crate-type in {}", manifest.display()))?;
    let patched = format!("{}[\"rlib\"]{}", &text[..lb], &text[rb + 1..]);
    if patched != text {
        std::fs::write(manifest, patched.as_bytes())
            .map_err(|e| format!("patch {}: {e}", manifest.display()))?;
    }
    Ok(())
}

/// Wire the selected distribution plugins into the `renzora_static_plugins`
/// aggregator (in the export copy) so the lean build links them in (no dlopen).
/// Returns whether any were wired (false = none selected, or none had source).
/// No restore: it operates on the disposable copy.
fn wire_static_plugins(
    crates_dir: &Path,
    crate_names: &[String],
    progress: &mut dyn FnMut(String),
) -> Result<bool, String> {
    if crate_names.is_empty() {
        return Ok(false);
    }
    let mut wired: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for name in crate_names {
        let manifest = crates_dir.join(name).join("Cargo.toml");
        if !manifest.is_file() {
            // Source not present (e.g. a marketplace plugin not yet downloaded).
            missing.push(name.clone());
            continue;
        }
        // Make the plugin linkable as an rlib (its cdylib stays too — harmless in
        // the copy).
        let bytes = std::fs::read(&manifest)
            .map_err(|e| format!("read {}: {e}", manifest.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        if !text.contains("rlib") {
            let patched = text.replacen("\"cdylib\"]", "\"cdylib\", \"rlib\"]", 1);
            if patched == *text {
                return Err(format!(
                    "Could not make {} linkable: unexpected crate-type format",
                    manifest.display()
                ));
            }
            std::fs::write(&manifest, patched.as_bytes())
                .map_err(|e| format!("patch {}: {e}", manifest.display()))?;
        }
        wired.push(name.clone());
    }

    if !missing.is_empty() {
        progress(format!(
            "Skipping {} plugin(s) with no in-workspace source (not embeddable yet): {}",
            missing.len(),
            missing.join(", ")
        ));
    }
    if wired.is_empty() {
        return Ok(false);
    }

    // Regenerate the aggregator. It's committed empty, so it must already exist.
    let agg_dir = crates_dir.join("renzora_static_plugins");
    let agg_manifest = agg_dir.join("Cargo.toml");
    let agg_lib = agg_dir.join("src").join("lib.rs");
    if !agg_manifest.is_file() || !agg_lib.is_file() {
        return Err(
            "renzora_static_plugins aggregator crate is missing from the workspace".into(),
        );
    }

    let deps: String = wired
        .iter()
        .map(|n| format!("{n} = {{ path = \"../{n}\", default-features = false }}\n"))
        .collect();
    let manifest = format!(
        "[package]\nname = \"renzora_static_plugins\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [lib]\ncrate-type = [\"rlib\"]\n\n[dependencies]\n{deps}\n[lints]\nworkspace = true\n"
    );
    let externs: String = wired.iter().map(|n| format!("extern crate {n};\n")).collect();
    let lib = format!(
        "//! GENERATED in the export copy by the renzora_export lean build.\n\
         #![allow(unused_extern_crates)]\n{externs}"
    );
    std::fs::write(&agg_manifest, manifest.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(&agg_lib, lib.as_bytes()).map_err(|e| e.to_string())?;

    progress(format!(
        "Statically linking {} plugin(s): {}",
        wired.len(),
        wired.join(", ")
    ));
    Ok(true)
}
