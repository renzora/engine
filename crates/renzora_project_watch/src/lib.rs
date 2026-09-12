//! One filesystem watcher over the open project.
//!
//! This crate owns the answer to "did a file change?" for the whole engine. It
//! watches the project root, publishes [`ProjectFileChanged`] for every change,
//! and forwards asset-shaped changes into `AssetServer` so Bevy's own
//! hot-reload works. Nothing else needs a watcher, a poll timer, or an mtime
//! comparison.
//!
//! The reasoning for centralising this, and the history of the six separate
//! pollers it replaces, is in [`renzora::core::project_files`]. What follows is
//! what this crate does about it.
//!
//! # Bevy's watcher, used from the outside
//!
//! The watcher is Bevy's own [`FileWatcher`], so there is no second `notify`
//! backend in the process and no second debouncer to tune. What Bevy does not
//! offer is a way to *observe* the resulting stream: `AssetSourceEvent`s go
//! into a single-consumer channel that `handle_internal_asset_events` drains,
//! and they only ever become reloads for assets that are already loaded. A file
//! that no `AssetServer` handle points at, which includes every `.rs` script, is
//! dropped on the floor.
//!
//! So this crate drives the watcher and Bevy listens to us, rather than the
//! other way round: we construct `FileWatcher` with a channel we own, publish
//! what arrives as an ECS event, and push the asset-shaped part of it into the
//! sender the engine's asset source captured for exactly this purpose (see
//! `renzora_engine::setup_asset_reader`).
//!
//! # Why the watcher is rebuilt rather than re-pointed
//!
//! `notify` has no "watch this other root instead" that is cheaper than
//! dropping the watcher, and a project switch is a rare, already-expensive
//! operation. Dropping it also drops every path it had queued, which is what we
//! want: events from the project the user just closed are noise.
//!
//! [`FileWatcher`]: bevy::asset::io::file::FileWatcher

use std::path::{Path, PathBuf};

use bevy::asset::io::AssetSourceEvent;
use bevy::prelude::*;
use renzora::core::project_files::{
    is_ignored, is_transient, AssetKind, AssetReloadSink, FileChange, ProjectFileChanged,
};
use renzora::core::CurrentProject;

/// How long `FileWatcher` coalesces events before handing them over.
///
/// Bevy's own default is 300ms, which is tuned for "a texture was saved, reload
/// it" and is slower than it needs to be for an editor sitting next to the
/// files. 100ms still absorbs the multi-syscall write that a single save
/// produces, which is the thing debouncing is actually for: without it, one
/// Ctrl+S in an external editor arrives as a create, one or more writes and a
/// close, and a script would be recompiled several times over.
const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(100);

/// One watched directory.
///
/// `_watcher` is an opaque handle whose only job is to stay alive: dropping it
/// stops the watching. It is `Option` because a root that cannot be watched (a
/// path that does not exist, a filesystem `notify` has no backend for) is kept
/// in the list anyway, so that `sync_roots` does not retry it every frame.
struct Watched {
    root: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    _watcher: Option<bevy::asset::io::file::FileWatcher>,
    /// Our end of this watcher's channel. Drained once per frame.
    receiver: Option<async_channel::Receiver<AssetSourceEvent>>,
}

/// Every directory being watched: the open project, plus whatever
/// [`ExtraWatchRoots`] asked for.
///
/// A watcher per root rather than one over a common ancestor, because there
/// often is no useful common ancestor: the project lives wherever the user put
/// it and the engine's `languages/` folder sits beside the executable, so the
/// nearest shared parent is easily a whole drive.
#[derive(Resource, Default)]
struct ProjectWatcher {
    watched: Vec<Watched>,
}

#[derive(Default)]
pub struct ProjectWatchPlugin;

impl Plugin for ProjectWatchPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ProjectWatchPlugin");
        app.init_resource::<ProjectWatcher>()
            .add_message::<ProjectFileChanged>()
            // `PreUpdate` so a consumer reading in `Update` sees the change on
            // the same frame it arrived rather than the next one. Bevy's own
            // `handle_internal_asset_events` also runs in `PreUpdate`, and the
            // forward into it is a channel push, so ordering against it does
            // not matter: a push that lands after it has run is picked up on
            // the following frame, which is indistinguishable at 100ms
            // debounce.
            .init_resource::<renzora::core::project_files::ExtraWatchRoots>()
            .add_systems(PreUpdate, (sync_roots, drain).chain());
    }
}

renzora::add!(ProjectWatchPlugin, Editor);

/// Keep the set of watchers matching the project plus [`ExtraWatchRoots`].
///
/// Roots that are already watched are left alone, so this is a list comparison
/// on a handful of paths per frame and nothing else. A root that goes away has
/// its watcher dropped, which matters on Windows: a live watcher holds a
/// directory handle, and a project the user just closed would refuse to be moved
/// or renamed until the handle went with it.
fn sync_roots(
    project: Option<Res<CurrentProject>>,
    extra: Res<renzora::core::project_files::ExtraWatchRoots>,
    mut watcher: ResMut<ProjectWatcher>,
) {
    let wanted: Vec<PathBuf> = project
        .map(|p| p.path.clone())
        .into_iter()
        .chain(extra.paths().iter().cloned())
        .collect();

    if watcher.watched.len() == wanted.len()
        && watcher
            .watched
            .iter()
            .zip(&wanted)
            .all(|(w, want)| w.root == *want)
    {
        return;
    }

    // Rebuilt in `wanted` order, reusing the watchers that survive. The project
    // is always first, so a project switch moves exactly one entry and leaves
    // the extra roots' watchers untouched.
    let mut kept: Vec<Watched> = std::mem::take(&mut watcher.watched);
    for want in wanted {
        if let Some(i) = kept.iter().position(|w| w.root == want) {
            let existing = kept.remove(i);
            watcher.watched.push(existing);
        } else {
            watcher.watched.push(start(want));
        }
    }
    // Whatever is left in `kept` is no longer wanted; dropping it here stops
    // those watchers.
}

#[cfg(not(target_arch = "wasm32"))]
fn start(root: PathBuf) -> Watched {
    use bevy::asset::io::file::FileWatcher;

    // Unbounded, and deliberately so. A bounded channel would have to decide
    // what to do when a `git checkout` rewrites ten thousand files at once, and
    // every answer is wrong: blocking stalls the watcher thread, and dropping
    // loses the one event some consumer needed. The debouncer already collapses
    // bursts, and an event is two paths and an enum.
    let (sender, receiver) = async_channel::unbounded();
    match FileWatcher::new(root.clone(), sender, DEBOUNCE) {
        Ok(w) => {
            info!("[project-watch] watching {}", root.display());
            Watched {
                root,
                _watcher: Some(w),
                receiver: Some(receiver),
            }
        }
        Err(e) => {
            // Not fatal, and kept in the list rather than retried: everything
            // downstream degrades to "you have to reopen the project to see
            // external edits", which is what the editor did before this crate
            // existed.
            //
            // A root that does not exist is expected rather than wrong, and is
            // the common case: `ExtraWatchRoots` holds candidate locations, and
            // `renzora_lang` registers both `<exe>/languages` and
            // `<cwd>/languages` because it does not know which one an install
            // put them in. A dev build has the exe under `dist/` with no
            // `languages/` beside it, so warning here meant a scary line on
            // every single launch about a directory nobody expected to be there.
            if !root.exists() {
                debug!("[project-watch] nothing at {}, not watching it", root.display());
            } else {
                warn!(
                    "[project-watch] cannot watch {}: {e}. Changes there will not be picked up.",
                    root.display()
                );
            }
            Watched {
                root,
                _watcher: None,
                receiver: None,
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn start(root: PathBuf) -> Watched {
    // Web has no filesystem to watch, and Bevy refuses the `file_watcher`
    // feature there outright. The entry is still recorded so `sync_roots` does
    // not retry it every frame.
    Watched {
        root,
        receiver: None,
    }
}

/// Turn everything the watcher saw this frame into events.
fn drain(
    watcher: Res<ProjectWatcher>,
    sink: Option<Res<AssetReloadSink>>,
    project: Option<Res<CurrentProject>>,
    mut events: MessageWriter<ProjectFileChanged>,
) {
    for watched in &watcher.watched {
        let Some(receiver) = &watched.receiver else {
            continue;
        };
        // Only the project's own events go to `AssetServer`. Its paths are
        // resolved against the project asset source, so handing it a path under
        // the engine's `languages/` folder would have it look for that path
        // inside the project and reload nothing, or worse, the wrong thing.
        let is_project = project
            .as_ref()
            .is_some_and(|p| p.path == watched.root);

        // `try_recv` in a loop rather than draining with a timeout: this runs on
        // the main thread and must never wait on the disk.
        while let Ok(raw) = receiver.try_recv() {
            // Forwarded verbatim, before our own filtering. The asset server has
            // its own idea of what it cares about (it reloads only paths it
            // already holds a handle for), and second-guessing that here would
            // be how an asset quietly stops hot-reloading.
            if is_project {
                if let Some(sink) = sink.as_ref() {
                    sink.send(raw.clone());
                }
            }
            for event in translate(raw, &watched.root) {
                log_event(&event);
                events.write(event);
            }
        }
    }
}

/// Print every published change, with `--watch-log` or `RENZORA_WATCH_LOG=1`.
///
/// Off by default and deliberately not a `debug!`: turning the whole crate up to
/// debug in `RUST_LOG` is a different and much noisier request than "show me
/// what the watcher is seeing", which is the question anyone actually has here.
///
/// The question it answers is "did my edit reach the engine at all?", and that
/// has exactly two wrong answers to tell apart: the watcher never saw the file
/// (nothing prints, so look at ignore rules and watched roots), or it saw it and
/// whatever should have reacted did not (a line prints and nothing follows it).
/// Without this both look identical from the outside.
///
/// A flag as well as an environment variable, and the flag is the one to reach
/// for. `cargo renzora` launches the editor as a grandchild process, so an
/// environment variable has to be exported in the right shell, in the right
/// syntax, and survive two hops to get here: easy to believe you have set and
/// not have. `--watch-log` is visible in the command you typed, and xtask
/// forwards trailing arguments (`cargo renzora run --watch-log`), exactly like
/// `--xr`.
fn log_event(event: &ProjectFileChanged) {
    // Resolved once. Both an env lookup and an `args()` walk are more than this
    // loop should do per event, and a `git checkout` can hand it thousands.
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        std::env::var_os("RENZORA_WATCH_LOG").is_some()
            || std::env::args().any(|a| a == "--watch-log")
    }) {
        return;
    }
    let what = match &event.change {
        FileChange::Added => "added".to_string(),
        FileChange::Modified => "modified".to_string(),
        FileChange::Removed => "removed".to_string(),
        FileChange::Renamed { from } => {
            format!("renamed from {}", from.to_string_lossy().replace('\\', "/"))
        }
    };
    info!(
        "[project-watch] {} {} ({:?}{})",
        what,
        event.relative,
        event.kind,
        if event.is_dir { ", dir" } else { "" }
    );
}

/// Convert one [`AssetSourceEvent`] into the events worth publishing.
///
/// Returns a `Vec` because a rename crosses the ignore boundary in both
/// directions: moving a file out of `target/` is an addition to everyone
/// watching, and moving one in is a removal, so a single rename can produce one
/// event, two, or none.
fn translate(raw: AssetSourceEvent, root: &Path) -> Vec<ProjectFileChanged> {
    // Bevy reports paths relative to the watched root already, but only for
    // paths under it. `strip_prefix` is belt and braces for the absolute ones
    // some notify backends produce.
    let rel = |p: &Path| -> String {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let make = |change: FileChange, path: PathBuf, is_dir: bool| -> Option<ProjectFileChanged> {
        let relative = rel(&path);
        if relative.is_empty() || is_ignored(&relative) || is_transient(&relative) {
            return None;
        }
        Some(ProjectFileChanged {
            change,
            kind: AssetKind::from_path(&path),
            path: root.join(&path),
            root: root.to_path_buf(),
            relative,
            is_dir,
        })
    };

    match raw {
        AssetSourceEvent::AddedAsset(p) => make(FileChange::Added, p, false).into_iter().collect(),
        AssetSourceEvent::ModifiedAsset(p) => {
            make(FileChange::Modified, p, false).into_iter().collect()
        }
        AssetSourceEvent::RemovedAsset(p) => {
            make(FileChange::Removed, p, false).into_iter().collect()
        }
        AssetSourceEvent::AddedFolder(p) => make(FileChange::Added, p, true).into_iter().collect(),
        AssetSourceEvent::RemovedFolder(p) => {
            make(FileChange::Removed, p, true).into_iter().collect()
        }
        AssetSourceEvent::RenamedAsset { old, new } => renamed(old, new, false, &make),
        AssetSourceEvent::RenamedFolder { old, new } => renamed(old, new, true, &make),
        // `RemovedUnknown` is notify telling us something went away without
        // saying whether it was a file or a folder, which happens for renames
        // out of an unwatched directory. Reported as a file removal: every
        // consumer that acts on a removal checks the path itself first (see
        // `ProjectFileChanged::still_missing`), so guessing wrong costs nothing,
        // while dropping it would lose a real deletion.
        AssetSourceEvent::RemovedUnknown { path, is_meta: false } => {
            make(FileChange::Removed, path, false).into_iter().collect()
        }
        // Bevy's `.meta` sidecars. Renzora writes none, and the asset server has
        // already been handed these above, so there is nothing here for a
        // consumer to act on.
        AssetSourceEvent::AddedMeta(_)
        | AssetSourceEvent::ModifiedMeta(_)
        | AssetSourceEvent::RemovedMeta(_)
        | AssetSourceEvent::RenamedMeta { .. }
        | AssetSourceEvent::RemovedUnknown { is_meta: true, .. } => Vec::new(),
    }
}

/// A rename, as seen by someone who ignores part of the tree.
///
/// Within the watched set it is one `Renamed`. Across the ignore boundary it is
/// an arrival or a departure, because that is what it looks like to a consumer:
/// a script dragged out of `target/` has appeared as far as anyone downstream is
/// concerned, and one dragged in has gone.
fn renamed(
    old: PathBuf,
    new: PathBuf,
    is_dir: bool,
    make: &impl Fn(FileChange, PathBuf, bool) -> Option<ProjectFileChanged>,
) -> Vec<ProjectFileChanged> {
    let from_visible = make(FileChange::Removed, old.clone(), is_dir).is_some();
    let to_visible = make(FileChange::Added, new.clone(), is_dir).is_some();
    match (from_visible, to_visible) {
        (true, true) => make(FileChange::Renamed { from: old }, new, is_dir)
            .into_iter()
            .collect(),
        (true, false) => make(FileChange::Removed, old, is_dir).into_iter().collect(),
        (false, true) => make(FileChange::Added, new, is_dir).into_iter().collect(),
        (false, false) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/proj")
    }

    #[test]
    fn a_modified_script_becomes_one_event() {
        let out = translate(
            AssetSourceEvent::ModifiedAsset(PathBuf::from("scripts/fps.rs")),
            &root(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative, "scripts/fps.rs");
        assert_eq!(out[0].kind, AssetKind::Script);
        assert_eq!(out[0].change, FileChange::Modified);
        assert!(out[0].is_live());
    }

    #[test]
    fn build_output_is_dropped() {
        let out = translate(
            AssetSourceEvent::ModifiedAsset(PathBuf::from("target/debug/thing.rs")),
            &root(),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_rename_inside_the_project_is_one_rename() {
        let out = translate(
            AssetSourceEvent::RenamedAsset {
                old: PathBuf::from("scripts/a.rs"),
                new: PathBuf::from("scripts/b.rs"),
            },
            &root(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative, "scripts/b.rs");
        assert!(matches!(out[0].change, FileChange::Renamed { .. }));
    }

    #[test]
    fn a_rename_out_of_an_ignored_dir_reads_as_an_addition() {
        let out = translate(
            AssetSourceEvent::RenamedAsset {
                old: PathBuf::from("target/a.rs"),
                new: PathBuf::from("scripts/a.rs"),
            },
            &root(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].change, FileChange::Added);
        assert_eq!(out[0].relative, "scripts/a.rs");
    }

    #[test]
    fn a_rename_into_an_ignored_dir_reads_as_a_removal() {
        let out = translate(
            AssetSourceEvent::RenamedAsset {
                old: PathBuf::from("scripts/a.rs"),
                new: PathBuf::from("target/a.rs"),
            },
            &root(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].change, FileChange::Removed);
        assert_eq!(out[0].relative, "scripts/a.rs");
    }

    #[test]
    fn an_atomic_saves_scratch_file_is_not_published() {
        for raw in [
            AssetSourceEvent::AddedAsset(PathBuf::from("p/x.particle.tmp.20124.8ed8f2")),
            AssetSourceEvent::ModifiedAsset(PathBuf::from("p/x.particle.tmp.20124.8ed8f2")),
        ] {
            assert!(translate(raw, &root()).is_empty());
        }
    }

    #[test]
    fn renaming_a_scratch_file_over_the_real_one_reads_as_an_arrival() {
        // The last step of an atomic save. The scratch half is invisible, so
        // this is not a rename to anyone downstream: the real file gained new
        // contents, and `is_live()` is what a consumer checks.
        let out = translate(
            AssetSourceEvent::RenamedAsset {
                old: PathBuf::from("p/x.particle.tmp.20124.8ed8f2"),
                new: PathBuf::from("p/x.particle"),
            },
            &root(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].change, FileChange::Added);
        assert_eq!(out[0].relative, "p/x.particle");
        assert!(out[0].is_live());
    }

    #[test]
    fn meta_sidecars_are_not_published() {
        let out = translate(
            AssetSourceEvent::ModifiedMeta(PathBuf::from("t.png.meta")),
            &root(),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn paths_are_absolute_and_relative_at_once() {
        let out = translate(
            AssetSourceEvent::AddedAsset(PathBuf::from("scenes/main.bsn")),
            &root(),
        );
        assert_eq!(out[0].path, root().join("scenes/main.bsn"));
        assert_eq!(out[0].relative, "scenes/main.bsn");
        assert_eq!(out[0].kind, AssetKind::Scene);
    }
}
