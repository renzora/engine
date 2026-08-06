//! Recompiles a plugin when its **source** changes, so editing a Rust file is a
//! live edit rather than a build step.
//!
//! [`super::loader`] already notices a changed `.dll` and swaps it in. This closes
//! the other half: watch `plugins/*/src`, run `cargo build`, drop the artifact
//! where the loader is looking, and let the existing path take it from there.
//! Four links, and only the first two are new:
//!
//! ```text
//! edit a .rs  →  cargo build  →  copy the dll  →  loader reloads it
//! ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~     ~~~~~~~~~~~~~~~~~
//!                    here                          already existed
//! ```
//!
//! This is what makes a standalone plugin usable the way a script is. It works at
//! all because such a plugin links no Bevy: a one-file change rebuilds in about a
//! second, where a Bevy-linking plugin would spend half a minute linking and the
//! loop would not be worth having.
//!
//! ## Why this lives in the host rather than an editor crate
//!
//! It is the same feature as the reload. The source watcher, the dll watcher and
//! the reload queue all answer "how does a plugin get into the running process",
//! and splitting them would have an editor crate reaching back into
//! [`super::loader::request_reload`] while both polled overlapping state.
//!
//! It costs this crate nothing to hold: spawning cargo is `std::process`, and the
//! reporting is a resource an editor crate reads. That direction is deliberate and
//! documented in `Cargo.toml` — a plugin author must be able to `cargo add
//! renzora_plugin`, so this crate never depends on the engine.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

/// How often to stat the source tree. Two polls settle a file, as in the dll
/// watcher — an editor writing a file is as partial as a linker writing a dll.
const POLL_INTERVAL: f32 = 0.25;

/// Which plugin sources to watch, and what they looked like last time.
#[derive(Resource)]
pub struct PluginSourceWatcher {
    /// Directory holding one subdirectory per plugin crate.
    pub root: PathBuf,
    /// Where to put a built library, i.e. the directory the loader scans.
    stage_to: PathBuf,
    seen: HashMap<PathBuf, (std::time::SystemTime, u64)>,
    settling: HashSet<PathBuf>,
    countdown: f32,
}

/// What a finished `cargo build` produced.
#[derive(Clone, Debug)]
pub struct PluginBuildResult {
    pub plugin: String,
    pub ok: bool,
    /// cargo's own rendered output. Empty on success.
    ///
    /// Text rather than parsed diagnostics because this is what a human reads, and
    /// it needs no dependency. `--message-format=json` would give file/line spans
    /// for inline markers in the code editor — worth adding when something wants
    /// to render them, not before.
    pub output: String,
}

/// Builds in flight, and the outcome of the last one per plugin.
///
/// Public so an editor crate can show them however it likes. This crate only logs.
#[derive(Resource)]
pub struct PluginBuilds {
    /// Plugin names currently compiling. A second change while one is in flight is
    /// dropped rather than queued: cargo would serialise on its own lock anyway,
    /// and the newer source is what the *next* build picks up regardless.
    pub in_flight: HashSet<String>,
    pub results: Vec<PluginBuildResult>,
    tx: Sender<PluginBuildResult>,
    /// `Mutex` because a `Receiver` is `Send` but not `Sync`, and a Bevy resource
    /// must be both. Nothing contends for it — one system drains it — so the lock
    /// is bookkeeping to satisfy the bound, not synchronisation.
    rx: std::sync::Mutex<Receiver<PluginBuildResult>>,
}

impl Default for PluginBuilds {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            in_flight: HashSet::new(),
            results: Vec::new(),
            tx,
            rx: std::sync::Mutex::new(rx),
        }
    }
}

/// Find the directory holding plugin crates, or `None` to leave the watcher off.
///
/// Three candidates, in order:
///
/// 1. `RENZORA_PLUGIN_SRC`, for a layout none of the guesses fit.
/// 2. `<cwd>/plugins` — `cargo renzora` launches with the repo root as the working
///    directory, so this is the normal dev case.
/// 3. `<exe-dir>/../../plugins` — the staged layout is `dist/<platform>/`, so the
///    repo root is two levels up. Covers running the exe directly.
///
/// A shipped game has none of these and gets no source watcher, which is correct:
/// there is no toolchain to compile with and nothing to compile.
fn find_source_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("RENZORA_PLUGIN_SRC") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
        warn!("RENZORA_PLUGIN_SRC is set but not a directory — ignoring it");
    }
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("plugins");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.parent()?.parent()?.join("plugins");
    candidate.is_dir().then_some(candidate)
}

/// Install the source watcher. Editor-only, called by the loader's plugin.
pub(crate) fn install(app: &mut App, stage_to: PathBuf) {
    let Some(root) = find_source_root() else {
        debug!("[plugin] no plugin source directory found — live rebuild is off");
        return;
    };
    info!("[plugin] watching {} for source changes", root.display());
    app.insert_resource(PluginSourceWatcher {
        // Seeded on the first poll rather than here: an empty map plus the "unseen
        // means changed" rule below would rebuild every plugin at startup.
        seen: HashMap::new(),
        settling: HashSet::new(),
        countdown: POLL_INTERVAL,
        root,
        stage_to,
    })
    .init_resource::<PluginBuilds>()
    .add_systems(Last, (poll_plugin_sources, drain_plugin_builds));
}

/// Every file whose change should trigger a rebuild, for one plugin crate.
///
/// `Cargo.toml` counts — adding a dependency changes what compiles just as much as
/// editing a function. `target/` is skipped for the obvious reason: cargo writes
/// there while building, and watching it would make every build trigger the next.
fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                source_files(&path, out);
            }
        } else if name == "Cargo.toml" || path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn poll_plugin_sources(
    time: Res<Time>,
    mut watcher: ResMut<PluginSourceWatcher>,
    mut builds: ResMut<PluginBuilds>,
) {
    watcher.countdown -= time.delta_secs();
    if watcher.countdown > 0.0 {
        return;
    }
    watcher.countdown = POLL_INTERVAL;

    let Ok(entries) = std::fs::read_dir(&watcher.root) else {
        return;
    };
    // Collected first because the loop below mutates `watcher`.
    let crates: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            Some((name, p))
        })
        .collect();

    let first_poll = watcher.seen.is_empty();
    for (name, dir) in crates {
        let mut files = Vec::new();
        source_files(&dir, &mut files);

        let mut changed = false;
        for file in files {
            let Ok(meta) = std::fs::metadata(&file) else { continue };
            let Ok(mtime) = meta.modified() else { continue };
            let stamp = (mtime, meta.len());
            match watcher.seen.get(&file).copied() {
                // Unseen. On the first poll that is just "this file exists" — the
                // staged build is already current, and rebuilding everything at
                // startup would be a minute of cargo for nothing.
                None => {
                    watcher.seen.insert(file.clone(), stamp);
                    if !first_poll {
                        watcher.settling.insert(file);
                    }
                }
                Some(prev) if prev != stamp => {
                    watcher.seen.insert(file.clone(), stamp);
                    watcher.settling.insert(file);
                }
                // Unchanged since the last poll. If it moved then, the write is
                // done and this plugin is ready to rebuild.
                Some(_) => {
                    if watcher.settling.remove(&file) {
                        changed = true;
                    }
                }
            }
        }

        if !changed || builds.in_flight.contains(&name) {
            continue;
        }
        info!("[plugin] {name} source changed, rebuilding");
        builds.in_flight.insert(name.clone());
        spawn_build(builds.tx.clone(), name, dir, watcher.stage_to.clone());
    }
}

/// Run `cargo build` on a worker thread and stage the result.
///
/// A thread rather than a Bevy task because the work is a blocking child process
/// with no `await` points, and it must not hold up a frame — a cold build is
/// seconds even for a crate this small.
///
/// Staging happens HERE, on the worker, rather than back on the main thread: the
/// copy is the trigger for the dll watcher, so doing it off-thread means the frame
/// that receives the result has nothing left to do.
fn spawn_build(tx: Sender<PluginBuildResult>, plugin: String, dir: PathBuf, stage_to: PathBuf) {
    std::thread::spawn(move || {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        // `dist` to match what the editor itself was staged with, so the artifact
        // lands in `target/dist/` where the copy below looks for it.
        let output = std::process::Command::new(cargo)
            .current_dir(&dir)
            .args(["build", "--profile", "dist"])
            .output();

        let result = match output {
            Err(e) => PluginBuildResult {
                plugin,
                ok: false,
                output: format!("could not run cargo: {e}"),
            },
            Ok(out) if !out.status.success() => PluginBuildResult {
                plugin,
                ok: false,
                // stderr, not stdout: cargo's diagnostics go there.
                output: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Ok(_) => match stage(&dir, &stage_to) {
                Ok(()) => PluginBuildResult {
                    plugin,
                    ok: true,
                    output: String::new(),
                },
                Err(e) => PluginBuildResult {
                    plugin,
                    ok: false,
                    output: format!("built, but could not stage it: {e}"),
                },
            },
        };
        // The receiver is a resource in a world that outlives this thread in every
        // normal case; on shutdown it may be gone, and a failed send is then the
        // correct outcome rather than something to report.
        let _ = tx.send(result);
    });
}

/// Copy whatever libraries the build produced into the directory the loader scans.
///
/// Sweeps `target/dist` rather than assuming the file is named after the
/// directory, because a crate's library name need not match its folder.
fn stage(dir: &Path, stage_to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(stage_to)?;
    let suffix = format!(".{}", std::env::consts::DLL_EXTENSION);

    // Two possible artifact directories, and the shared one is now the normal
    // case. `plugins/.cargo/config.toml` sets `target-dir = "target"`, which
    // cargo resolves relative to THAT file's directory — so every plugin builds
    // into `plugins/target/`, not `plugins/<name>/target/`. Looking only in the
    // per-plugin location failed with a bare "cannot find the path" for every
    // plugin, because that directory has not existed since the shared target dir
    // was introduced as a build-time optimisation.
    //
    // The per-plugin path is still tried first: a plugin built outside this
    // checkout has one, and it is the more specific answer when both exist.
    let own = dir.join("target").join("dist");
    let shared = dir
        .parent()
        .map(|p| p.join("target").join("dist"))
        .unwrap_or_else(|| own.clone());
    let (from, shared_dir) = if own.is_dir() {
        (own, false)
    } else {
        (shared, true)
    };

    // Every plugin's artifacts sit in the shared directory, so copying the lot
    // would restage all of them on every keystroke — and would overwrite a
    // plugin the editor has loaded with whatever happened to be built last. Take
    // only this one, matched on the directory name.
    let want = dir
        .file_name()
        .map(|n| format!("{}{suffix}", n.to_string_lossy()));

    let mut copied = 0;
    for entry in std::fs::read_dir(&from)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        if !name.to_string_lossy().ends_with(&suffix) {
            continue;
        }
        // Only filter in the shared directory. A per-plugin `target/` holds one
        // plugin's output, and its library name need not match its folder.
        if shared_dir {
            match &want {
                Some(w) if name.to_string_lossy() != *w => continue,
                None => continue,
                _ => {}
            }
        }
        std::fs::copy(&path, stage_to.join(&name))?;
        copied += 1;
    }
    if copied == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            // Naming what was looked for, not just where: in the shared
            // directory the usual cause is a package name that differs from the
            // folder name, and "no .dll in plugins/target/dist" is actively
            // misleading when 60 of them are sitting there.
            match &want {
                Some(w) => format!("no {w} in {} — is it a cdylib, and does the package name match the folder?", from.display()),
                None => format!("no {suffix} in {} — is it a cdylib?", from.display()),
            },
        ));
    }
    Ok(())
}

fn drain_plugin_builds(mut builds: ResMut<PluginBuilds>) {
    // Poison-tolerant: a panic on the build thread while holding this would
    // otherwise stop every later build being reported, and the panic has already
    // surfaced on its own.
    let finished: Vec<PluginBuildResult> = match builds.rx.lock() {
        Ok(rx) => rx.try_iter().collect(),
        Err(e) => e.into_inner().try_iter().collect(),
    };
    for result in finished {
        builds.in_flight.remove(&result.plugin);
        if result.ok {
            // No "reloaded" message here — the dll watcher logs that when it picks
            // the file up, which is also the moment it is actually true.
            info!("[plugin] {} rebuilt", result.plugin);
        } else {
            // Deliberately the whole of cargo's output. A compile error truncated
            // to one line is a worse experience than a long log line, and the
            // reload is refused either way, so nothing is broken while you read it.
            error!("[plugin] {} failed to build:\n{}", result.plugin, result.output);
        }
        // Keep only the most recent result per plugin: this is a status display,
        // not a history, and an unbounded Vec in a long editor session is a leak.
        builds.results.retain(|r| r.plugin != result.plugin);
        builds.results.push(result);
    }
}
