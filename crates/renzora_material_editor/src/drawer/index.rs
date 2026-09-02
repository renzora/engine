//! The cached list of the project's `.material` files that feeds the picker.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, IoTaskPool, Task};

use renzora::core::CurrentProject;

use crate::material_inspector::find_material_files;

/// Cached list of the project's `.material` files, feeding the picker popup.
///
/// The scan is a recursive `read_dir` walk of the project (see
/// [`find_material_files`]). It used to run inline in the picker rebuild, which
/// rebuilds on **every keystroke** in the search box — so typing one character
/// walked the whole project. Profiling put that path at 13.9 ms in a single
/// frame. The walk now runs on the IO task pool and publishes here.
///
/// Same shape and same reasoning as `renzora_inspector`'s `ScriptIndex`: there is
/// no file-watch signal to hook (`.material` files are read with raw `std::fs`,
/// never through the `AssetServer`), so a slow throttle catches files created by
/// anything other than the editor itself.
#[derive(Resource, Default)]
pub(crate) struct MaterialIndex {
    /// Last completed scan: `(project-relative path, absolute path)`, sorted.
    /// `Arc` so the picker snapshots it without cloning every entry per rebuild.
    pub(super) materials: Arc<Vec<(String, String)>>,
    /// Bumped only when `materials` actually changes content. The picker's keyed
    /// list folds this into its dirty token, so a periodic rescan that finds
    /// nothing new re-snapshots nothing.
    pub(super) generation: u64,
    /// Project root the cached scan came from; a change rescans immediately.
    root: Option<PathBuf>,
    /// `Time::elapsed_secs()` when the in-flight walk *started*, so a slow walk
    /// can't immediately trigger the next one. Wall-clock rather than an
    /// accumulated delta because this system is gated on a popup being open.
    last_scan: Option<f32>,
    /// The walk in flight. Never dropped to "cancel" — dropping a bevy `Task`
    /// cancels the work — it is held until `poll_once` yields.
    task: Option<Task<Vec<(String, String)>>>,
}

/// How often to re-walk for `.material` files created by something other than the
/// editor. Matches `ScriptIndex`'s throttle; the popup is short-lived, so in
/// practice this is one walk per time it is opened.
const MATERIAL_SCAN_THROTTLE: f32 = 3.0;

/// Land a finished walk and start a new one when the project changed or the
/// throttle elapsed. Bumps [`MaterialIndex::generation`] only when the file set
/// actually changed, so a rescan that finds nothing new rebuilds nothing.
pub(super) fn refresh_material_index(
    mut index: ResMut<MaterialIndex>,
    project: Option<Res<CurrentProject>>,
    time: Res<Time>,
) {
    // Bind the poll result before touching `index.task` again — folding this into
    // the `if let` keeps the `as_mut()` borrow alive across the body.
    let finished = index.task.as_mut().and_then(|t| block_on(poll_once(t)));
    if let Some(materials) = finished {
        index.task = None;
        // Only republish when the set really changed: the generation bump makes
        // the picker re-snapshot, and a rebuilt row loses its thumbnail binding,
        // so a periodic no-op rescan must not churn the list under the user.
        if materials != *index.materials {
            index.materials = Arc::new(materials);
            index.generation = index.generation.wrapping_add(1);
        }
    }

    let Some(project) = project else { return };
    if index.task.is_some() {
        return;
    }

    let now = time.elapsed_secs();
    let root_changed = index.root.as_deref() != Some(project.path.as_path());
    let stale = index.last_scan.is_none_or(|t| now - t >= MATERIAL_SCAN_THROTTLE);
    if !root_changed && !stale {
        return;
    }
    if root_changed {
        index.root = Some(project.path.clone());
        index.materials = Arc::new(Vec::new());
    }
    index.last_scan = Some(now);

    let root = project.path.clone();
    index.task = Some(IoTaskPool::get().spawn(async move { find_material_files(&root) }));
}
