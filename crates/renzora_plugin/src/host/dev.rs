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
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};

/// How long a path must be quiet before its change counts.
///
/// An editor saving a file produces several filesystem events for one logical
/// write, and a `cargo build` produces thousands. The debouncer coalesces both
/// into one notification per path, which is what the old two-poll `settling` set
/// was approximating.
const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);

/// Which plugin sources to watch.
///
/// **This used to poll.** It re-walked every plugin crate's entire source tree
/// with a recursive `read_dir` every 0.25 s and diffed a map of `(mtime, len)`
/// stamps. Profiling put that at **~19 ms per walk, ~4 times a second** — 6.9 s
/// of CPU across a 96 s capture, on the main thread, in `Last`, on a splash
/// screen with no project open. It ran in every editor session (the install is
/// gated on `is_editor`, not on Dev Mode), and it was pure overhead in the
/// overwhelmingly common case where no source file had changed at all.
///
/// Now the OS says which path changed. Idle cost is draining an empty channel,
/// and the rebuild also *starts sooner* — no waiting out a poll interval.
#[derive(Resource)]
pub struct PluginSourceWatcher {
    /// Directory holding one subdirectory per plugin crate.
    pub root: PathBuf,
    /// Debounced filesystem events, drained each frame.
    rx: std::sync::Mutex<Receiver<DebounceEventResult>>,
    /// Crates we already hold watches for, so a new one can be picked up without
    /// re-adding watches for the rest.
    watched: HashSet<String>,
    /// Dropping the debouncer unregisters every watch, so it is kept alive here.
    /// `Option` so a failed install degrades to "no live rebuild" rather than
    /// taking the editor down; see [`install`].
    debouncer: Option<SourceDebouncer>,
}

type SourceDebouncer = notify_debouncer_full::Debouncer<
    notify_debouncer_full::notify::RecommendedWatcher,
    notify_debouncer_full::RecommendedCache,
>;

/// Watch one crate's sources: `src/` recursively, plus its manifest.
///
/// **Deliberately not a recursive watch on the crate directory**, because that
/// would include `plugins/<crate>/target/` — each plugin declares its own
/// `[workspace]`, so it has one. Every rebuild writes thousands of files there,
/// which would flood the notification queue with our own build output and can
/// overflow it. Filtering those paths after the fact (as [`is_source`] does)
/// is not enough: the events still have to be produced, queued and delivered,
/// and an overflow loses *real* events alongside the noise.
fn watch_crate(debouncer: &mut SourceDebouncer, dir: &Path) -> notify_debouncer_full::notify::Result<()> {
    let src = dir.join("src");
    if src.is_dir() {
        debouncer.watch(&src, RecursiveMode::Recursive)?;
    }
    let manifest = dir.join("Cargo.toml");
    if manifest.is_file() {
        debouncer.watch(&manifest, RecursiveMode::NonRecursive)?;
    }
    Ok(())
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
pub(crate) fn install(app: &mut App) {
    let Some(root) = find_source_root() else {
        debug!("[plugin] no plugin source directory found — live rebuild is off");
        return;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    // A failed watch is not fatal — Linux inotify has a per-user watch limit that
    // a large tree can exhaust, and network filesystems often emit nothing at all.
    // Losing live rebuild is a much better outcome than refusing to start, so this
    // degrades instead of propagating.
    let mut watched = HashSet::new();
    let debouncer = match new_debouncer(DEBOUNCE, None, tx) {
        Ok(mut d) => {
            // The root itself, non-recursively: this is what notices a plugin
            // crate being *added* while the editor runs, which the old poll got
            // for free by re-reading the root every tick.
            match d.watch(&root, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    // The one and only directory listing. Everything after this is
                    // event-driven; there is no baseline to maintain, because the
                    // OS reports what changed rather than us diffing what is there.
                    for (name, dir) in crate_dirs(&root) {
                        if let Err(e) = watch_crate(&mut d, &dir) {
                            warn!("[plugin] could not watch {name} ({e})");
                            continue;
                        }
                        watched.insert(name);
                    }
                    info!(
                        "[plugin] watching {} crate(s) under {} for source changes",
                        watched.len(),
                        root.display()
                    );
                    Some(d)
                }
                Err(e) => {
                    warn!(
                        "[plugin] could not watch {} ({e}) — live rebuild is off",
                        root.display()
                    );
                    None
                }
            }
        }
        Err(e) => {
            warn!("[plugin] could not start the source watcher ({e}) — live rebuild is off");
            None
        }
    };

    app.insert_resource(PluginSourceWatcher {
        root,
        rx: std::sync::Mutex::new(rx),
        watched,
        debouncer,
    })
    .init_resource::<PluginBuilds>()
    .add_systems(Last, (poll_plugin_sources, drain_plugin_builds));
}

/// Does a changed path mean "recompile this crate"?
///
/// `Cargo.toml` counts — adding a dependency changes what compiles just as much
/// as editing a function. `target/` is excluded for the obvious reason: cargo
/// writes there while building, and reacting to it would make every build
/// trigger the next one.
fn is_source(path: &Path) -> bool {
    if path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == "target" || s.starts_with('.')
    }) {
        return false;
    }
    match path.file_name().and_then(|n| n.to_str()) {
        Some("Cargo.toml") => true,
        _ => path.extension().and_then(|e| e.to_str()) == Some("rs"),
    }
}

/// Every plugin crate directly under `root`, as `(name, dir)`.
///
/// The single directory listing this module performs, done once at [`install`]
/// and again only when the root reports a new entry. A directory without a
/// manifest is not a crate — don't spawn cargo in it.
fn crate_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .filter(|p| !is_native_plugin(p))
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            Some((name, p))
        })
        .collect()
}

/// Whether `dir` is a NATIVE plugin rather than a C-ABI one.
///
/// Both kinds live in `plugins/`, and a native one is also a directory with a
/// `Cargo.toml` — so without this check it would land here and be rebuilt with
/// `cargo build`. That is precisely the wrong command for it: a native plugin
/// links Bevy, `plugins/` is outside the engine workspace, and cargo would
/// resolve it a FRESH Bevy from crates.io. The result builds cleanly, loads, and
/// corrupts the World, because its `TypeId`s do not match the host's.
///
/// Native plugins are rebuilt against the staged SDK instead — by `xtask` in the
/// dev tree, and by the editor's install flow on a user's machine. Neither is
/// this watcher's business.
///
/// Matched on the quoted `"dylib"`: `"cdylib"` also ends in `dylib`, and a loose
/// match would exclude every C-ABI plugin from hot reload.
fn is_native_plugin(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|text| {
        text.lines()
            .filter(|l| l.trim_start().starts_with("crate-type"))
            .any(|l| l.contains("\"dylib\""))
    })
}

/// Which plugin crate owns a changed path — the first directory under the root.
///
/// Returns `None` for a path outside the root or one lying directly in it (a
/// stray file next to the crate directories belongs to no crate).
fn owning_crate(root: &Path, path: &Path) -> Option<(String, PathBuf)> {
    let rel = path.strip_prefix(root).ok()?;
    let first = rel.components().next()?;
    let dir = root.join(first);
    // A directory with no manifest is not a crate — don't spawn cargo in it.
    // Nor is a native plugin, which has one but must never be built with cargo
    // (see `is_native_plugin`).
    (dir.join("Cargo.toml").is_file() && !is_native_plugin(&dir))
        .then(|| (first.as_os_str().to_string_lossy().into_owned(), dir))
}

/// Drain debounced filesystem events and rebuild whatever changed.
///
/// Keeps the name it had when it polled, because it is still the same link in
/// the chain the module doc describes; it just no longer does the walking.
fn poll_plugin_sources(mut watcher: ResMut<PluginSourceWatcher>, mut builds: ResMut<PluginBuilds>) {
    // One crate may produce many events for one save even after debouncing (a
    // `Cargo.toml` plus an `.rs`, say), so collect before spawning.
    let mut dirty: Vec<(String, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // A root-level event means a crate may have appeared; re-list only then.
    let mut root_changed = false;

    let Ok(rx) = watcher.rx.lock() else {
        return;
    };
    for batch in rx.try_iter() {
        let events: Vec<DebouncedEvent> = match batch {
            Ok(events) => events,
            // The OS queue overflowed — a `git checkout` or a cargo build can
            // outrun it. Events were dropped, so we cannot know what changed.
            // Log rather than guess: rebuilding everything on an overflow would
            // turn a branch switch into a full rebuild of every plugin.
            Err(errors) => {
                for e in errors {
                    warn!("[plugin] source watch error: {e}");
                }
                continue;
            }
        };
        for event in events {
            for path in &event.paths {
                if path.parent() == Some(watcher.root.as_path()) {
                    root_changed = true;
                }
                if !is_source(path) {
                    continue;
                }
                let Some((name, dir)) = owning_crate(&watcher.root, path) else {
                    continue;
                };
                if seen.insert(name.clone()) {
                    dirty.push((name, dir));
                }
            }
        }
    }
    drop(rx);

    // A crate appeared since we installed watches — cover it from now on. It is
    // NOT rebuilt here: a plugin that just showed up has whatever artifact it
    // shipped with, and rebuilding on discovery would fire cargo for every crate
    // the first time this ran.
    if root_changed {
        let known: Vec<(String, PathBuf)> = crate_dirs(&watcher.root)
            .into_iter()
            .filter(|(name, _)| !watcher.watched.contains(name))
            .collect();
        for (name, dir) in known {
            let Some(debouncer) = watcher.debouncer.as_mut() else {
                break;
            };
            match watch_crate(debouncer, &dir) {
                Ok(()) => {
                    info!("[plugin] now watching new crate {name}");
                    watcher.watched.insert(name);
                }
                Err(e) => warn!("[plugin] could not watch new crate {name} ({e})"),
            }
        }
    }

    for (name, dir) in dirty {
        // A second change while one is in flight is dropped rather than queued —
        // see `PluginBuilds::in_flight`.
        if builds.in_flight.contains(&name) {
            continue;
        }
        // An event is not a change. The debouncer reports what the filesystem
        // told it, and "the filesystem mentioned this path" covers a great deal
        // more than "the author edited it": a watch registered over an existing
        // tree, an editor writing a file back byte-identical, a `git checkout`
        // that restores what was already there.
        //
        // Left unchecked that is not one wasted build, it is one per plugin, all
        // at once. An install now holds the source of every plugin it has, so
        // launching the editor from its own directory put a watch over ~65
        // crates and rebuilt every one of them two seconds after startup —
        // 65 concurrent `cargo` processes, and then 65 hot reloads as each
        // artefact landed. That reads as the editor stuttering on startup, with
        // nothing on screen connecting it to a plugin.
        //
        // So ask the same question the build pass asks: is the source actually
        // newer than what was built from it?
        if !source_newer_than_artefact(&dir, &name) {
            debug!("[plugin] {name} reported a change but is not newer than its build — skipping");
            continue;
        }
        info!("[plugin] {name} source changed, rebuilding");
        builds.in_flight.insert(name.clone());
        spawn_build(builds.tx.clone(), name, dir);
    }
}

/// Is anything under `<dir>/src` newer than the library already built from it?
///
/// The same test `renzora_native_plugin::layout` uses to decide a plugin is
/// stale, asked here so a filesystem event that changed nothing costs nothing.
/// A missing artefact counts as "newer": there is nothing to compare against and
/// building is the right answer.
fn source_newer_than_artefact(dir: &Path, name: &str) -> bool {
    let artefact = dir
        .join("build")
        .join(format!("{}.{}", name.replace('-', "_"), std::env::consts::DLL_EXTENSION));
    let Ok(built) = std::fs::metadata(&artefact).and_then(|m| m.modified()) else {
        return true;
    };
    fn any_newer(dir: &Path, built: std::time::SystemTime) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                return any_newer(&p, built);
            }
            e.metadata().and_then(|m| m.modified()).map(|t| t > built).unwrap_or(false)
        })
    }
    any_newer(&dir.join("src"), built)
        || std::fs::metadata(dir.join("Cargo.toml"))
            .and_then(|m| m.modified())
            .map(|t| t > built)
            .unwrap_or(false)
}

/// Where `cargo` is, when `PATH` does not say.
///
/// Rustup installs into `~/.cargo/bin` and adds it to `PATH` from the shell's
/// profile — so an editor started from a terminal finds cargo and an editor
/// started from a desktop launcher does not. Spawning the bare name reports
/// "Rust is not installed" on a machine that plainly has it.
///
/// `CARGO` first, because cargo sets it for everything it spawns: an editor
/// launched by `cargo renzora` then rebuilds with the toolchain that built it.
///
/// Duplicated rather than shared with `renzora_native_build::tool`, because this
/// crate takes no dependencies at all — a plugin author must be able to
/// `cargo add renzora_plugin` — and fifteen lines is a smaller price than an
/// edge in that graph.
fn cargo_path() -> PathBuf {
    if let Some(explicit) = std::env::var_os("CARGO") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return path;
        }
    }
    let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cargo")));
    if let Some(candidate) = home.map(|h| h.join("bin").join(exe)) {
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("cargo")
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
fn spawn_build(tx: Sender<PluginBuildResult>, plugin: String, dir: PathBuf) {
    std::thread::spawn(move || {
        let cargo = cargo_path();
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
            Ok(_) => match stage(&dir) {
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

/// Copy the library the build produced into the plugin's own `build/` directory.
///
/// `plugins/<id>/build/<id>.<ext>`, which is where [`super::loader::artefacts`]
/// looks and where `renzora_native_plugin` puts a native plugin's library. One
/// layout for both kinds: the directory name is the plugin's identity, and the
/// artefact is named after it rather than after whatever cargo happened to call
/// the file.
///
/// That renaming is load-bearing rather than tidiness. Cargo emits a cdylib as
/// `lib<crate>.so` on Unix and `<crate>.dll` on Windows, so a staged file kept
/// under cargo's name gives the same plugin two ids depending on the platform —
/// and the disable list, the reload queue and the marketplace sidecar all key on
/// that id.
fn stage(dir: &Path) -> std::io::Result<()> {
    let ext = std::env::consts::DLL_EXTENSION;
    let Some(id) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "plugin directory has no name",
        ));
    };
    let crate_name = id.replace('-', "_");

    // Two possible artifact directories, and the shared one is the normal case:
    // a `plugins/.cargo/config.toml` setting `target-dir = "target"` resolves it
    // relative to THAT file's directory, so every plugin builds into
    // `plugins/target/`. The per-plugin path is tried first because a plugin
    // built outside such a tree has one, and it is the more specific answer.
    let own = dir.join("target").join("dist");
    let from = if own.is_dir() {
        own
    } else {
        dir.parent()
            .map(|p| p.join("target").join("dist"))
            .unwrap_or(own)
    };

    // What cargo named it. `DLL_PREFIX` is `lib` on Unix and empty on Windows —
    // spelling it out rather than sweeping the directory, because the shared
    // target dir holds every plugin's output and a sweep would restage all of
    // them, overwriting libraries the editor has already loaded.
    let produced = from.join(format!("{}{crate_name}.{ext}", std::env::consts::DLL_PREFIX));
    if !produced.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "no {} in {} — is it a cdylib, and does the package name match the folder?",
                produced.file_name().unwrap_or_default().to_string_lossy(),
                from.display()
            ),
        ));
    }

    // Written to a sibling and renamed in, never through the path the loader
    // reads — see the note on the copy helper. Inlined rather than shared with
    // `renzora_native_build::stage_atomically`, because this crate takes no
    // dependencies: a plugin author must be able to `cargo add renzora_plugin`.
    let build = dir.join("build");
    std::fs::create_dir_all(&build)?;
    let out = build.join(format!("{crate_name}.{ext}"));
    let want = std::fs::metadata(&produced)?.len();
    let tmp = out.with_extension("staging");
    let copied = std::fs::copy(&produced, &tmp)?;
    if copied != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("{copied} of {want} bytes — the library was still being written"),
        ));
    }
    std::fs::rename(&tmp, &out)?;
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
