//! Every click the window answers, and the verdict the worker is waiting on.

use bevy::prelude::*;
use renzora_ember::UiInteraction;

use crate::overlay::{close_overlay, ImportOverlayState, ImportProgress};

use super::tree::{subtree_of, tree_row_parts};
use super::{
    CancelBtn, CommitBtn, DiscardStagedBtn, FileBrowseBtn, FolderBrowseBtn, ImportNav, MatRow,
    MeshRow, ReconvertBtn, RemoveFileBtn, StagedRow, TabBtn, TreeCheck, TreeItem, TreeRow,
};

/// Turn ember's folder pick into the import's target directory.
///
/// [`FolderPick`](renzora_ember::widgets::FolderPick) is a single global
/// resource shared by every picker in the editor and it outlives the overlay
/// that seeded it, so this is deliberately narrow: only while the import window
/// is up, and only for a path that is actually inside the open project. A pick
/// left behind by the marketplace's install dialog must not silently repoint an
/// import.
pub(super) fn dest_folder_sync(
    pick: Option<Res<renzora_ember::widgets::FolderPick>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let (Some(pick), Some(project), Some(state)) = (pick, project, state.as_mut()) else {
        return;
    };
    if !state.visible {
        return;
    }
    let Some(path) = pick.0.as_deref() else { return };
    let Ok(rel) = path.strip_prefix(&project.path) else {
        return;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    if state.target_directory != rel {
        state.target_directory = rel;
    }
}

pub(super) fn tab_click(
    q: Query<(&UiInteraction, &TabBtn), Changed<UiInteraction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, t) in &q {
        if *i == UiInteraction::Pressed {
            nav.tab = t.0;
        }
    }
}

/// Clicking a tree row selects it; clicking an openable one also toggles it,
/// which is what a single-column tree in a narrow pane wants — a 9px caret is
/// too small to be the only way to expand.
pub(super) fn tree_row_click(
    q: Query<(&UiInteraction, &TreeRow), Changed<UiInteraction>>,
    mut nav: Option<ResMut<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
) {
    let (Some(nav), Some(state)) = (nav.as_mut(), state) else {
        return;
    };
    let stats = state.current().and_then(|s| s.stats.as_ref());
    for (i, r) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        nav.sel_item = Some(r.0);
        // Selecting a surface also points the Materials tab at its material, so
        // the two views agree about what you are looking at.
        if let (TreeItem::Prim(mi, k), Some(stats)) = (r.0, stats) {
            nav.sel_material = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .and_then(|p| p.material);
        }
        if let (TreeItem::Mesh(mi), Some(_)) = (r.0, stats) {
            nav.sel_mesh = Some(mi);
        }
        let expandable = stats
            .map(|st| tree_row_parts(st, r.0).3)
            .unwrap_or(false);
        if expandable && !nav.expanded.insert(r.0) {
            nav.expanded.remove(&r.0);
        }
    }
}

/// Tick or untick a part of the model.
///
/// Unticking is a subtree operation: the node goes, and any entry its children
/// had of their own goes with it — they are implied now, and dropping them is
/// what lets ticking the parent again restore the whole branch in one click,
/// which is what "check the parent, check the children" has to mean for the box
/// to be worth having.
pub(super) fn tree_check_click(
    q: Query<(&UiInteraction, &TreeCheck), Changed<UiInteraction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    let Some(item) = q
        .iter()
        .find(|(i, _)| **i == UiInteraction::Pressed)
        .map(|(_, c)| c.0)
    else {
        return;
    };
    let Some(stats) = state.current().and_then(|s| s.stats.clone()) else {
        return;
    };
    let active = state.active;
    let Some(staged) = state.staged.get_mut(active) else {
        return;
    };
    let ex = &mut staged.excluded;
    match item {
        TreeItem::Node(i) => {
            let subtree = subtree_of(&stats, i);
            let was_included = !ex.nodes.contains(&i);
            for &n in &subtree {
                ex.nodes.remove(&n);
                if let Some(mi) = stats.node_list.get(n).and_then(|n| n.mesh) {
                    ex.meshes.remove(&mi);
                    ex.prims.retain(|(m, _)| *m != mi);
                }
            }
            if was_included {
                ex.nodes.insert(i);
            }
        }
        TreeItem::Mesh(mi) => {
            let was_included = !ex.meshes.contains(&mi);
            ex.prims.retain(|(m, _)| *m != mi);
            if was_included {
                ex.meshes.insert(mi);
            } else {
                ex.meshes.remove(&mi);
            }
        }
        TreeItem::Prim(mi, k) => {
            if !ex.prims.remove(&(mi, k)) {
                ex.prims.insert((mi, k));
            }
        }
    }
}

pub(super) fn mesh_row_click(
    q: Query<(&UiInteraction, &MeshRow), Changed<UiInteraction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == UiInteraction::Pressed {
            nav.sel_mesh = Some(r.0);
        }
    }
}

pub(super) fn mat_row_click(
    q: Query<(&UiInteraction, &MatRow), Changed<UiInteraction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == UiInteraction::Pressed {
            nav.sel_material = Some(r.0);
        }
    }
}

/// Switch the window to another staged model. Selections are per-file, so they
/// reset — index 4 in one model is unrelated to index 4 in the next.
///
/// The tab is deliberately left alone. Clicking a row used to jump to Scene,
/// which threw you out of the list you were working down every time you picked
/// the next file out of it — the row's own highlight and the preview changing
/// are the feedback that the click landed.
pub(super) fn staged_row_click(
    q: Query<(&UiInteraction, &StagedRow), Changed<UiInteraction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, r) in &q {
        if *i == UiInteraction::Pressed && r.0 < state.staged.len() && state.active != r.0 {
            state.active = r.0;
            if let Some(nav) = nav.as_mut() {
                nav.reset_selection();
            }
        }
    }
}

/// Act on one staged file and keep the window's selection valid, since whatever
/// takes its place at that index is a different model.
fn decide(world: &mut World, decision: crate::staged::PreviewDecision) {
    if world.resource::<ImportOverlayState>().staged.is_empty() {
        return;
    }
    crate::overlay::apply_decision(world, decision);
    if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
        nav.reset_selection();
    }
}

/// Import: take **every** staged file into the project, not just the one on
/// show.
///
/// One button per file was the old shape, and it made a batch import a row of
/// identical decisions the user had already made by queueing the files. What is
/// left is per-file *rejection* (the trash on each row), which is the choice
/// that actually differs between them.
pub(super) fn commit_click(
    q: Query<&UiInteraction, (With<CommitBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(|w: &mut World| {
            // The worker baked its paths in when it converted, so a set built
            // for another folder (or other settings) has to be rebuilt first or
            // Import lands it in the wrong place. See `commit_when_converted`.
            if staged_is_stale(w.resource::<ImportOverlayState>()) {
                crate::overlay::request_reimport(w);
                w.resource_mut::<ImportOverlayState>().commit_when_converted = true;
                return;
            }
            commit_all(w);
        });
    }
}

/// Does the staged set no longer describe what the window is showing?
fn staged_is_stale(s: &ImportOverlayState) -> bool {
    !s.staged.is_empty()
        && s.converted_with
            .as_ref()
            .is_some_and(|c| !c.matches(&s.settings, &s.target_directory, s.layout))
}

fn commit_all(w: &mut World) {
    // Always commit index 0: `apply_decision` removes what it takes, so the set
    // shortens under the loop rather than being walked.
    while !w.resource::<ImportOverlayState>().staged.is_empty() {
        w.resource_mut::<ImportOverlayState>().active = 0;
        decide(w, crate::staged::PreviewDecision::Commit);
    }
}

/// Finish an Import that had to reconvert first: commit once the rebuilt set is
/// all staged and nothing is still running.
///
/// The destination is checked again rather than trusted, because the user can
/// pick another folder while the reconvert runs; that is just one more rebuild.
/// A reconvert that staged nothing (every file failed) drops the request, so a
/// later conversion does not commit itself without being asked.
pub(super) fn commit_when_converted(world: &mut World) {
    let Some(s) = world.get_resource::<ImportOverlayState>() else {
        return;
    };
    if !s.commit_when_converted
        || s.reimport_requested
        || s.active_task.is_some()
        || !s.pending_files.is_empty()
    {
        return;
    }
    if s.staged.is_empty() {
        world.resource_mut::<ImportOverlayState>().commit_when_converted = false;
        return;
    }
    if staged_is_stale(s) {
        crate::overlay::request_reimport(world);
        return;
    }
    world.resource_mut::<ImportOverlayState>().commit_when_converted = false;
    commit_all(world);
}

/// The trash on a staged row: delete that one converted tree, leave the rest.
pub(super) fn discard_staged_click(
    q: Query<(&UiInteraction, &DiscardStagedBtn), Changed<UiInteraction>>,
    mut commands: Commands,
) {
    let Some(index) = q
        .iter()
        .find(|(i, _)| **i == UiInteraction::Pressed)
        .map(|(_, b)| b.0)
    else {
        return;
    };
    commands.queue(move |w: &mut World| {
        if index >= w.resource::<ImportOverlayState>().staged.len() {
            return;
        }
        w.resource_mut::<ImportOverlayState>().active = index;
        decide(w, crate::staged::PreviewDecision::Skip);
    });
}

/// Rebuild every staged file with the settings as they now stand.
pub(super) fn reconvert_click(
    q: Query<&UiInteraction, (With<ReconvertBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(|w: &mut World| {
            crate::overlay::request_reimport(w);
            if let Some(mut nav) = w.get_resource_mut::<ImportNav>() {
                nav.reset_selection();
            }
        });
    }
}

pub(super) fn cancel_click(
    q: Query<&UiInteraction, (With<CancelBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(|w: &mut World| close_overlay(w));
    }
}

pub(super) fn remove_file_click(
    q: Query<(&UiInteraction, &RemoveFileBtn), Changed<UiInteraction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, rm) in &q {
        if *i == UiInteraction::Pressed {
            state.pending_files.retain(|q| q.path != rm.0);
        }
    }
}

pub(super) fn file_browse_click(
    q: Query<&UiInteraction, (With<FileBrowseBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(|w: &mut World| { pick_and_queue_files(w); });
    }
}

pub(super) fn folder_browse_click(
    q: Query<&UiInteraction, (With<FolderBrowseBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(|w: &mut World| { pick_and_queue_folder(w); });
    }
}

/// Open the OS file picker (filtered to every importable kind) and append the
/// chosen files to the queue. Returns `true` if at least one new file was added.
/// Shared by the asset-browser Import trigger (`lib.rs`) and the window's own
/// **Files** button, so both honour the same filter and de-dup rules.
pub fn pick_and_queue_files(world: &mut World) -> bool {
    let Some(paths) = crate::kinds::pick_importable_files() else {
        return false;
    };
    let assets: Vec<_> = paths.into_iter().map(crate::kinds::QueuedAsset::flat).collect();
    world.resource_mut::<ImportOverlayState>().enqueue(&assets)
}

/// Queue files the caller already chose, with no OS dialog, for
/// [`renzora::core::ImportPick::Given`]. A folder among them is walked the
/// same way a dropped one is, and anything the importer cannot read is left
/// out rather than queued to fail.
pub fn queue_given_files(world: &mut World, paths: Vec<std::path::PathBuf>) -> bool {
    let assets: Vec<_> = paths
        .iter()
        .flat_map(|p| crate::kinds::expand_importables(p))
        .collect();
    world.resource_mut::<ImportOverlayState>().enqueue(&assets)
}

/// Open the OS folder picker, expand it (mirroring the source tree), and
/// append to the queue. A folder with nothing importable in it reports that in
/// the overlay's message line instead of leaving the button looking dead.
pub fn pick_and_queue_folder(world: &mut World) -> bool {
    let Some((dir, assets)) = crate::kinds::pick_importable_folder() else {
        return false;
    };
    if assets.is_empty() {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("that folder")
            .to_string();
        world.resource_mut::<ImportOverlayState>().progress =
            ImportProgress::Error(format!("No importable files in {}", name));
        return false;
    }
    world.resource_mut::<ImportOverlayState>().enqueue(&assets)
}

