//! Reloading the open scene when its `.bsn` changes outside the editor.
//!
//! Edit a scene file in a text editor, `git checkout` a branch, or have a tool
//! rewrite it, and the viewport follows without reopening the project. The
//! change arrives as a [`ProjectFileChanged`] from `renzora_project_watch`;
//! nothing here watches anything.
//!
//! # Why this is mostly refusals
//!
//! Reloading a scene throws the world away and builds a new one. That is the
//! right answer when the file genuinely changed underneath you and the wrong
//! answer in several situations that look identical from the outside, so most
//! of this module is about telling them apart:
//!
//! - **The editor's own save.** Saving writes the file, which the watcher
//!   reports as a change. Reloading there would discard selection and undo every
//!   time you pressed Ctrl+S, for a scene already identical to the one on
//!   screen. Caught by [`SelfWrites`], which compares content rather than
//!   timestamps.
//! - **Unsaved edits.** The file changed and so has the world, and there is no
//!   merge: one of them has to lose. The user's unsaved work is not ours to
//!   discard, so the reload is refused and the conflict is reported instead.
//! - **Play mode.** A reload mid-play would despawn the running game. Whatever
//!   the file says can wait for Stop.
//! - **A scene that is not open.** Editing `level2.bsn` while `level1.bsn` is on
//!   screen changes a file, not the thing you are looking at.
//!
//! # What it does not do
//!
//! Preserve selection across the reload, or merge. A reload is a reload: the
//! world is rebuilt from the file, and whatever was selected is gone with the
//! entities it pointed at. Re-selecting by name afterwards would be guesswork
//! about which `Cube` is the same `Cube`, and getting that subtly wrong is worse
//! than plainly losing the selection.

use bevy::prelude::*;
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};
use renzora::core::console_log::{console_info, console_warn};
use renzora::core::project_files::{AssetKind, ProjectFileChanged, SelfWrites};
use renzora::core::CurrentProject;

/// Scene files that changed on disk this frame, collected for the exclusive
/// system that acts on them.
///
/// A two-step because reloading needs `&mut World` and reading messages does
/// not: an exclusive system that also took a `MessageReader` would serialise the
/// whole schedule behind a queue that is empty almost every frame.
#[derive(Resource, Default)]
pub(crate) struct PendingSceneReloads(Vec<std::path::PathBuf>);

/// Collect `.bsn` changes. Cheap, runs every frame, refuses almost everything.
pub(crate) fn collect_scene_changes(
    mut pending: ResMut<PendingSceneReloads>,
    mut changes: MessageReader<ProjectFileChanged>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
) {
    if changes.is_empty() {
        return;
    }
    // Play mode first, and draining rather than leaving them: a change made
    // while the game is running should not be applied the moment it stops, when
    // the user is looking at something else entirely.
    if play_mode.is_some_and(|pm| pm.is_in_play_mode() || pm.is_simulating()) {
        changes.clear();
        return;
    }
    for change in changes.read() {
        if change.kind != AssetKind::Scene {
            continue;
        }
        // A save arrives as an unlink followed by a rename (the scratch file is
        // suppressed, so what reaches here is `Removed` then `Added`). The
        // removal half is not a deletion and must not be queued as one.
        if !change.is_live() {
            debug!("[scene] {} removed, not queuing a reload", change.relative);
            continue;
        }
        debug!("[scene] {} changed, queued for reload check", change.relative);
        if !pending.0.contains(&change.path) {
            pending.0.push(change.path.clone());
        }
    }
}

/// Reload the open scene if one of the changed files is it.
pub(crate) fn apply_scene_reloads(world: &mut World) {
    let paths = {
        let Some(mut pending) = world.get_resource_mut::<PendingSceneReloads>() else {
            return;
        };
        if pending.0.is_empty() {
            return;
        }
        std::mem::take(&mut pending.0)
    };

    for path in paths {
        // Every refusal below says so. A reload that does not happen is
        // indistinguishable from a watcher that never fired, and the whole
        // point of these guards is that most of the time refusing IS correct:
        // without a reason on the line, the correct behaviour and a bug look
        // exactly alike. At most one line per scene file change, which is rare.
        let shown = path.display().to_string();

        // Only the scene on screen. A scene open in a background tab is left
        // alone deliberately: reloading it would be invisible work now and a
        // surprise later, when switching to that tab shows something other than
        // what was left there.
        let Some((open_path, is_modified)) = active_scene(world) else {
            info!("[scene] {shown} changed on disk, but no scene tab is active");
            continue;
        };
        if !same_file(&path, &open_path) {
            info!(
                "[scene] {shown} changed on disk, but the open scene is {}",
                open_path.display()
            );
            continue;
        }

        // The editor's own write, echoed back. Not a no-op guard for speed: this
        // is the difference between Ctrl+S saving and Ctrl+S saving and then
        // wiping your selection.
        let Ok(bytes) = std::fs::read(&path) else {
            // Gone between the event and now. `FileChange::Removed` is handled
            // by not being `is_live`, so this is a file deleted moments after
            // being written, and there is nothing to load.
            info!("[scene] {shown} changed on disk, but cannot be read now");
            continue;
        };
        if world
            .get_resource::<SelfWrites>()
            .is_some_and(|s| s.matches(&path, &bytes))
        {
            info!("[scene] {shown} changed on disk, but the contents are the ones we wrote");
            continue;
        }

        let rel = world
            .get_resource::<CurrentProject>()
            .and_then(|p| p.make_relative(&path))
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        if is_modified {
            // Refused, and said loudly in both places: the Console scrolls, and
            // the Problems panel is where someone looks to ask whether the
            // project is healthy. Silence here would mean the editor quietly
            // showing one scene while the file held another.
            console_warn(
                "Scene",
                format!(
                    "{rel} changed on disk, but this scene has unsaved changes — not reloading. \
                     Save to keep your version, or reopen the scene to take the one on disk."
                ),
            );
            warn!("[scene] {rel} changed on disk but has unsaved changes; not reloading");
            set_conflict(world, &rel, true);
            continue;
        }

        console_info("Scene", format!("{rel} changed on disk, reloading"));
        info!("[scene] {rel} changed on disk, reloading");
        set_conflict(world, &rel, false);

        // Queued, NOT `scene_io::load_scene` directly.
        //
        // `load_scene` writes a scene into the world; it does not take the old
        // one out first. The despawn sweep that makes a load a *replacement*
        // lives in `process_pending_scene_loads`, one level up, along with
        // cancelling a half-streamed previous load. Calling the inner function
        // reloaded on top of what was already there: every reload spawned
        // another full copy of the scene, deduped into `cube_1`, `cube_2`,
        // `world_environment_1` and so on, growing without limit.
        //
        // Queueing by project-relative name is what a script's own scene switch
        // does, so this takes the identical path rather than a parallel one that
        // has to remember the same steps.
        world
            .resource_mut::<renzora::PendingSceneLoad>()
            .requests
            .push(rel.clone());
    }
}

/// The active scene tab's absolute path and whether it has unsaved changes.
fn active_scene(world: &World) -> Option<(std::path::PathBuf, bool)> {
    let tabs = world.get_resource::<renzora_ui::DocumentTabState>()?;
    let tab = tabs.tabs.get(tabs.active_tab)?;
    let rel = tab.scene_path.as_deref()?;
    let project = world.get_resource::<CurrentProject>()?;
    Some((project.resolve_path(rel), tab.is_modified))
}

/// Same file, allowing for the two paths having been spelled differently.
///
/// `canonicalize` because the watcher's path is built from the watched root
/// while the tab's comes from the project config, and on Windows those can
/// differ in case and in `\` versus `/` while naming one file. Falls back to a
/// plain comparison when either cannot be canonicalized, which is the case for a
/// file that has just been deleted.
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Put an unsaved-changes conflict on the file, or clear it.
fn set_conflict(world: &mut World, rel: &str, conflicted: bool) {
    let Some(mut problems) = world.get_resource_mut::<ContentProblems>() else {
        return;
    };
    let rows = if conflicted {
        vec![ContentProblem {
            severity: ProblemSeverity::Warning,
            line: None,
            message: "This scene changed on disk while you have unsaved changes. \
                      Saving overwrites the version on disk; reopening discards yours."
                .to_string(),
            node_id: None,
        }]
    } else {
        Vec::new()
    };
    // By severity, so a warning raised here does not take down errors another
    // producer put on the same file.
    problems.set_severity(rel.to_string(), ProblemSeverity::Warning, rows);
}
