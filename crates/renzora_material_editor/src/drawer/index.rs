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
/// Same shape and same reasoning as `renzora_inspector`'s `ScriptIndex`.
///
/// # The walk happens once per project, not once per change
///
/// The list is maintained **incrementally** from `ProjectFileChanged`. A file
/// event names exactly which `.material` arrived, moved or went away, and this
/// is a sorted list, so applying that is a binary search and a splice. The walk
/// exists only to answer the one question an event cannot: what was already on
/// disk when the project opened.
///
/// The alternative, re-walking on every change, was the first version of this
/// and it is the wasteful one: the walk's cost scales with the number of files
/// in the project rather than the number of materials in it, so importing a pack
/// of fifty materials into a large project would have paid fifty whole-project
/// walks to learn fifty things the events had already said.
#[derive(Resource, Default)]
pub(crate) struct MaterialIndex {
    /// `(project-relative path, absolute path)`, sorted by the relative path.
    /// `Arc` so the picker snapshots it without cloning every entry per rebuild.
    pub(super) materials: Arc<Vec<(String, String)>>,
    /// Bumped only when `materials` actually changes content. The picker's keyed
    /// list folds this into its dirty token, so an update that changes nothing
    /// re-snapshots nothing.
    pub(super) generation: u64,
    /// Project root the cached scan came from; a change rescans immediately.
    root: Option<PathBuf>,
    /// The walk in flight. Never dropped to "cancel" — dropping a bevy `Task`
    /// cancels the work — it is held until `poll_once` yields.
    task: Option<Task<Vec<(String, String)>>>,
}

impl MaterialIndex {
    /// Apply one file change to the sorted list. Returns whether it changed.
    ///
    /// Kept in agreement with `find_material_files` on purpose, and the depth
    /// bound is the subtle half: that walk stops at [`MATERIAL_SCAN_MAX_DEPTH`],
    /// so a material buried deeper is not in the list. Inserting one here
    /// regardless would make the list depend on whether you were watching when
    /// the file appeared, and it would vanish on the next project open.
    fn apply(&mut self, relative: &str, absolute: &str, exists: bool) -> bool {
        use crate::material_inspector::MATERIAL_SCAN_MAX_DEPTH;
        if relative.matches('/').count() >= MATERIAL_SCAN_MAX_DEPTH {
            return false;
        }
        let list = Arc::make_mut(&mut self.materials);
        match (list.binary_search_by(|(rel, _)| rel.as_str().cmp(relative)), exists) {
            (Err(at), true) => {
                list.insert(at, (relative.to_string(), absolute.to_string()));
                true
            }
            (Ok(at), false) => {
                list.remove(at);
                true
            }
            // Already present and still there, or already absent and still
            // gone. A `Modified` on an existing material lands here, which is
            // right: editing a material does not change which ones exist.
            _ => false,
        }
    }
}

/// Land a finished walk, and start one when the project opens or a `.material`
/// file appears, moves or goes away.
///
/// Keep the index current as `.material` files come and go.
///
/// Ungated, unlike the walk below, and that is required rather than tidy: a
/// `MessageReader` inside a `run_if` advances no cursor while the condition is
/// false, and the messages it skipped are dropped after two frames. Gated, the
/// picker would open knowing nothing about materials created while it was
/// closed. This costs a buffer read per frame, which on a quiet disk is reading
/// that there is nothing to read.
///
/// A three-second throttle used to re-walk the whole project for as long as the
/// popup was open, and the first fix for that re-walked on every change instead.
/// Both did work proportional to the size of the project to learn something the
/// event already said; this applies the event directly.
pub(super) fn track_material_files(
    mut index: ResMut<MaterialIndex>,
    project: Option<Res<CurrentProject>>,
    mut changes: MessageReader<renzora::core::project_files::ProjectFileChanged>,
) {
    use renzora::core::project_files::{AssetKind, FileChange};

    if changes.is_empty() {
        return;
    }
    // Nothing to be relative to, and the opening walk has not run: whatever is
    // in the buffer will be covered by it.
    let Some(project) = project else {
        changes.clear();
        return;
    };
    if index.root.as_deref() != Some(project.path.as_path()) {
        changes.clear();
        return;
    }

    let mut changed = false;
    for change in changes.read() {
        // A rename moves an entry: the old path leaves the list and the new one
        // joins it, and either end may be a `.material` without the other being
        // one (renaming `a.material` to `a.txt` is a removal).
        if let FileChange::Renamed { from } = &change.change {
            let old_rel = from
                .strip_prefix(&project.path)
                .unwrap_or(from)
                .to_string_lossy()
                .replace('\\', "/");
            if AssetKind::from_path(from) == AssetKind::Material {
                changed |= index.apply(&old_rel, "", false);
            }
        }
        if change.kind != AssetKind::Material {
            continue;
        }
        let absolute = change.path.to_string_lossy().to_string();
        // `still_missing` rather than trusting the event: an editor that saves
        // by writing a temp file and renaming over the original produces a
        // removal for a file that is still very much there, and acting on it
        // would drop a material out of the picker mid-save.
        let exists = change.is_live() || !change.still_missing();
        changed |= index.apply(&change.relative, &absolute, exists);
    }

    if changed {
        index.generation = index.generation.wrapping_add(1);
    }
}

/// Land the opening walk: what was already on disk when the project opened,
/// which no file event will ever mention.
pub(super) fn refresh_material_index(
    mut index: ResMut<MaterialIndex>,
    project: Option<Res<CurrentProject>>,
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

    // Only on a project change now. Steady state is `track_material_files`,
    // which needs no walk at all.
    if index.root.as_deref() == Some(project.path.as_path()) {
        return;
    }
    // Switching projects invalidates the old list outright: clear it now rather
    // than offering the previous project's materials until the walk lands.
    index.root = Some(project.path.clone());
    index.materials = Arc::new(Vec::new());

    let root = project.path.clone();
    index.task = Some(IoTaskPool::get().spawn(async move { find_material_files(&root) }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(paths: &[&str]) -> MaterialIndex {
        MaterialIndex {
            materials: Arc::new(
                paths
                    .iter()
                    .map(|p| (p.to_string(), format!("/proj/{p}")))
                    .collect(),
            ),
            ..Default::default()
        }
    }

    fn names(index: &MaterialIndex) -> Vec<String> {
        index.materials.iter().map(|(r, _)| r.clone()).collect()
    }

    #[test]
    fn an_added_material_lands_in_sorted_position() {
        let mut i = index_of(&["a.material", "c.material"]);
        assert!(i.apply("b.material", "/proj/b.material", true));
        assert_eq!(names(&i), ["a.material", "b.material", "c.material"]);
    }

    #[test]
    fn a_removed_material_leaves() {
        let mut i = index_of(&["a.material", "b.material"]);
        assert!(i.apply("a.material", "", false));
        assert_eq!(names(&i), ["b.material"]);
    }

    #[test]
    fn adding_one_that_is_already_listed_changes_nothing() {
        let mut i = index_of(&["a.material"]);
        assert!(!i.apply("a.material", "/proj/a.material", true));
        assert_eq!(names(&i), ["a.material"]);
    }

    #[test]
    fn removing_one_that_was_never_listed_changes_nothing() {
        let mut i = index_of(&["a.material"]);
        assert!(!i.apply("zz.material", "", false));
        assert_eq!(names(&i), ["a.material"]);
    }

    #[test]
    fn a_material_deeper_than_the_walk_reaches_is_not_listed() {
        // `find_material_files` stops at MATERIAL_SCAN_MAX_DEPTH, so accepting
        // this incrementally would put an entry in the list that vanishes the
        // next time the project is opened.
        let mut i = index_of(&[]);
        assert!(!i.apply("a/b/c/d/e/f/g.material", "/proj/deep", true));
        assert!(names(&i).is_empty());
    }

    #[test]
    fn incremental_matches_a_full_walk() {
        // The property that matters: applying events one at a time must land
        // the same list a walk would produce, or the picker's contents depend
        // on whether you were watching when the files appeared.
        let mut incremental = index_of(&[]);
        for p in ["m/b.material", "a.material", "m/a.material"] {
            incremental.apply(p, &format!("/proj/{p}"), true);
        }
        incremental.apply("a.material", "", false);

        let mut walked: Vec<String> = vec!["m/b.material".into(), "m/a.material".into()];
        walked.sort();
        assert_eq!(names(&incremental), walked);
    }
}
