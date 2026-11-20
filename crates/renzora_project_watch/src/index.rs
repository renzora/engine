//! Keeps `renzora::ProjectIndex` current.
//!
//! The first listing of a project is a walk of every folder in it, slow for a
//! big one, so it never runs in a frame: the companion process does it when
//! connected (job `"project_index"`, which also tells the companion which
//! project to watch for its caches), and a thread of the editor's own does it
//! otherwise, or if the companion could not. From then on the index follows
//! this crate's own [`ProjectFileChanged`] events, which already carry every
//! change: nobody walks the project again.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use renzora::core::project_files::ProjectFileChanged;
use renzora::core::project_index::{walk_project, IndexChange, IndexedFile, ProjectIndex};
use renzora::core::{CurrentProject, FileChange};
use renzora::job_args::ProjectIndexWalk;

/// The walk that is out, if one is.
#[derive(Resource, Default)]
struct Walk {
    root: Option<PathBuf>,
    /// Handed to the companion.
    job: Option<renzora::CompanionJobId>,
    /// Running on a thread here.
    local: Option<Mutex<Receiver<Vec<IndexedFile>>>>,
}

pub(crate) fn build(app: &mut App) {
    renzora::init_companion_jobs(app);
    app.init_resource::<ProjectIndex>()
        .init_resource::<Walk>()
        // After `drain`, so a change and the walk that might miss it land in
        // order: see `ProjectIndex::complete`.
        .add_systems(PreUpdate, (begin_walk, finish_walk, follow_changes).chain().after(super::drain));
}

/// Start a walk when a project opens (or the open one changes).
fn begin_walk(
    project: Option<Res<CurrentProject>>,
    mut index: ResMut<ProjectIndex>,
    mut walk: ResMut<Walk>,
    jobs: Option<ResMut<renzora::CompanionJobs>>,
) {
    let Some(project) = project else { return };
    if index.root() == Some(project.path.as_path()) {
        return;
    }
    index.begin(project.path.clone());
    *walk = Walk { root: Some(project.path.clone()), ..default() };
    let args = ProjectIndexWalk { root: project.path.clone() };
    let target = project.config.name.clone();
    match jobs.and_then(|mut j| j.submit("project_index", target, &args)) {
        Some(id) => walk.job = Some(id),
        None => walk.local = Some(Mutex::new(walk_here(project.path.clone()))),
    }
}

/// Walk on a thread of the editor's own.
fn walk_here(root: PathBuf) -> Receiver<Vec<IndexedFile>> {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new().name("project-index".into()).spawn(move || {
        let _ = tx.send(walk_project(&root));
    });
    if let Err(e) = spawned {
        warn!("[project-index] could not start the walk: {e}");
    }
    rx
}

/// Take the walk's result, from the companion or from the thread. A
/// companion that could not walk is done over here.
fn finish_walk(
    mut done: MessageReader<renzora::CompanionJobDone>,
    mut walk: ResMut<Walk>,
    mut index: ResMut<ProjectIndex>,
) {
    let Some(root) = walk.root.clone() else {
        done.clear();
        return;
    };
    for answer in done.read() {
        if walk.job != Some(answer.id) {
            continue;
        }
        walk.job = None;
        match answer.result.as_deref().map(serde_json::from_str::<Vec<IndexedFile>>) {
            Ok(Ok(files)) => index.complete(&root, files),
            failed => {
                let why = match failed {
                    Err(e) => e.to_string(),
                    Ok(Err(e)) => format!("its answer could not be read: {e}"),
                    Ok(Ok(_)) => unreachable!(),
                };
                warn!("[project-index] the companion could not list the project ({why}); listing it here");
                walk.local = Some(Mutex::new(walk_here(root.clone())));
            }
        }
    }
    let local = walk.local.as_ref().and_then(|rx| rx.lock().ok().and_then(|rx| rx.try_recv().ok()));
    if let Some(files) = local {
        walk.local = None;
        index.complete(&root, files);
    }
}

/// Apply the watcher's changes to the open project.
fn follow_changes(
    mut changes: MessageReader<ProjectFileChanged>,
    project: Option<Res<CurrentProject>>,
    mut index: ResMut<ProjectIndex>,
) {
    let Some(project) = project else {
        changes.clear();
        return;
    };
    for change in changes.read() {
        if change.root != project.path {
            continue;
        }
        // A rename is the old path gone as well as the new one arriving.
        if let FileChange::Renamed { from } = &change.change {
            if let Ok(old) = from.strip_prefix(&change.root) {
                index.apply(IndexChange::Remove(old.to_string_lossy().replace('\\', "/")));
            }
        }
        match change.change {
            FileChange::Removed => index.apply(IndexChange::Remove(change.relative.clone())),
            _ if change.is_dir => {}
            _ => {
                if let Some(file) = IndexedFile::stat(change.relative.clone(), &change.path) {
                    index.apply(IndexChange::Upsert(file));
                }
            }
        }
    }
}
