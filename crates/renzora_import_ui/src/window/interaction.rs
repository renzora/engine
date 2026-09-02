//! Every click the window answers, and the verdict the worker is waiting on.

use bevy::prelude::*;

use crate::overlay::{close_overlay, ImportOverlayState, ImportProgress};

use super::tree::{subtree_of, tree_row_parts};
use super::{
    CancelBtn, CommitBtn, DestFolderRow, DiscardAllBtn, FileBrowseBtn, FolderBrowseBtn, ImportNav,
    ImportTab, MatRow, MeshRow, RemoveFileBtn, SkipBtn, StagedRow, TabBtn, TreeCheck, TreeItem,
    TreeRow,
};

pub(super) fn tab_click(
    q: Query<(&Interaction, &TabBtn), Changed<Interaction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, t) in &q {
        if *i == Interaction::Pressed {
            nav.tab = t.0;
        }
    }
}

/// Clicking a tree row selects it; clicking an openable one also toggles it,
/// which is what a single-column tree in a narrow pane wants — a 9px caret is
/// too small to be the only way to expand.
pub(super) fn tree_row_click(
    q: Query<(&Interaction, &TreeRow), Changed<Interaction>>,
    mut nav: Option<ResMut<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
) {
    let (Some(nav), Some(state)) = (nav.as_mut(), state) else {
        return;
    };
    let stats = state.current().and_then(|s| s.stats.as_ref());
    for (i, r) in &q {
        if *i != Interaction::Pressed {
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
    q: Query<(&Interaction, &TreeCheck), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    let Some(item) = q
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
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
    q: Query<(&Interaction, &MeshRow), Changed<Interaction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed {
            nav.sel_mesh = Some(r.0);
        }
    }
}

pub(super) fn mat_row_click(
    q: Query<(&Interaction, &MatRow), Changed<Interaction>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed {
            nav.sel_material = Some(r.0);
        }
    }
}

/// Switch the window to another staged model. Selections are per-file, so they
/// reset — index 4 in one model is unrelated to index 4 in the next.
pub(super) fn staged_row_click(
    q: Query<(&Interaction, &StagedRow), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed && r.0 < state.staged.len() {
            let changed = state.active != r.0;
            state.active = r.0;
            if let Some(nav) = nav.as_mut() {
                if changed {
                    nav.reset_selection();
                }
                // Always move to Scene, so clicking a row that is already
                // active still does something visible rather than sitting dead.
                nav.tab = ImportTab::Scene;
            }
        }
    }
}

/// Answer the blocked worker and reset the window back to its file-picking
/// state, since the next staged file (if any) arrives fresh.
fn decide(world: &mut World, decision: crate::staged::PreviewDecision) {
    if world.resource::<ImportOverlayState>().staged.is_empty() {
        return;
    }
    crate::overlay::apply_decision(world, decision);
    if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
        nav.reset_selection();
    }
}

pub(super) fn commit_click(
    q: Query<&Interaction, (With<CommitBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::Commit));
    }
}

pub(super) fn skip_click(
    q: Query<&Interaction, (With<SkipBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::Skip));
    }
}

pub(super) fn discard_all_click(
    q: Query<&Interaction, (With<DiscardAllBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::CancelAll));
    }
}

pub(super) fn cancel_click(
    q: Query<&Interaction, (With<CancelBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| close_overlay(w));
    }
}

pub(super) fn remove_file_click(
    q: Query<(&Interaction, &RemoveFileBtn), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, rm) in &q {
        if *i == Interaction::Pressed {
            state.pending_files.retain(|q| q.path != rm.0);
        }
    }
}

pub(super) fn file_browse_click(
    q: Query<&Interaction, (With<FileBrowseBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| { pick_and_queue_files(w); });
    }
}

pub(super) fn folder_browse_click(
    q: Query<&Interaction, (With<FolderBrowseBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
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

/// Click a destination folder row → it becomes the import target directory.
pub(super) fn dest_folder_click(
    q: Query<(&Interaction, &DestFolderRow), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, row) in &q {
        if *i == Interaction::Pressed && state.target_directory != row.0 {
            state.target_directory = row.0.clone();
        }
    }
}
