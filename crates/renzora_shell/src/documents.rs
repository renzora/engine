//! Moving between open documents: activating one, keeping the workspace and the
//! active document in step, following renamed files, and closing.
//!
//! **There is no tab strip.** The open documents are listed in the top bar's
//! scene switcher (see [`crate::scene_switcher`]), which names the active one as
//! `project / document`. A strip used to run under the top bar, and before that
//! inside the viewport panel; most of the time it held one or two tabs and spent
//! a whole row of the window on them, and a Settings option offered a dropdown
//! in its place. The switcher already was that dropdown, so both went.
//!
//! Scenes and assets share the one list.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::dock::{Dock, DockDirty};

use crate::ribbon::{apply_workspace, select_workspace};
use crate::save_prompts::{CloseTabPromptRoot, TabCloseRequest};
use crate::ShellLayouts;

/// Most-recently-active document ids, oldest first, so a workspace switch can
/// return to the document you were last in *there* (see
/// [`sync_active_doc_to_workspace`]).
///
/// Kept here rather than in [`renzora_ui::DocumentTabState`] because it's a
/// property of this session's navigation, not of the document set: nothing
/// persists it, and every activation route (the switcher, an asset browser
/// double-click, the inspector's edit button) is observed the same way: by
/// [`sync_workspace_to_active_doc`] noticing the active document changed.
#[derive(Resource, Default)]
pub(crate) struct DocTabMru(Vec<u64>);

/// Activate document `id` and switch to the workspace its kind opens in, from a
/// `&mut World` context such as a menu row's action.
///
/// The `TabSwitchRequest` is what swaps the content: without it a scene to
/// scene move only switched the dock layout (a no-op) and the viewport kept the
/// old scene. `handle_tab_switch` caches the leaving document and restores this
/// one's buffered scene.
pub(crate) fn activate_doc_tab(w: &mut World, id: u64) {
    let Some(mut state) = w.get_resource_mut::<renzora_ui::DocumentTabState>() else {
        return;
    };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let switch = state.activate_tab(idx);
    let layout = state.tabs[idx].kind.layout_name().map(|n| n.to_string());
    if let Some((old_tab_id, new_tab_id)) = switch {
        w.insert_resource(renzora::TabSwitchRequest { old_tab_id, new_tab_id });
    }
    let Some(layout) = layout else { return };
    let index = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.iter().position(|(n, _)| *n == layout));
    if let Some(index) = index {
        select_workspace(w, index);
    }
}

/// Whether document `id` may be closed. The model refuses the last document
/// overall *and* the last scene, the latter so Asset mode always has a scene to
/// return to; offering a close it would quietly decline is worse than none.
pub(crate) fn doc_closable(state: &renzora_ui::DocumentTabState, id: u64) -> bool {
    let scenes = state.tabs.iter().filter(|t| !t.kind.is_asset()).count();
    state.tabs.len() > 1
        && state
            .tabs
            .iter()
            .find(|t| t.id == id)
            .is_some_and(|t| t.kind.is_asset() || scenes > 1)
}

/// Close document `id` from a `&mut World` context. A document with unsaved
/// changes opens the save-confirmation prompt instead of closing outright (see
/// [`crate::save_prompts::process_tab_close_request`]); a clean one closes now.
pub(crate) fn request_close_doc(world: &mut World, id: u64) {
    // A prompt is already up: it owns the next decision.
    let prompt_open = world
        .query_filtered::<(), With<CloseTabPromptRoot>>()
        .iter(world)
        .next()
        .is_some();
    if prompt_open {
        return;
    }
    let modified = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.iter().find(|t| t.id == id))
        .map(|t| t.is_modified);
    match modified {
        None => {}
        Some(true) => world.insert_resource(TabCloseRequest { id }),
        Some(false) => {
            let mut queue = CommandQueue::default();
            world.resource_scope(|world, mut state: Mut<renzora_ui::DocumentTabState>| {
                let mut commands = Commands::new(&mut queue, world);
                close_doc_tab_by_id(&mut state, id, &mut commands);
            });
            queue.apply(world);
        }
    }
}

/// Whether a document of `kind` belongs to the workspace called `workspace`.
///
/// Both layout tables count. `layout_name` is the direct answer for most kinds,
/// but shaders name a `Shaders` workspace that doesn't exist. The layout that
/// actually opens a `.wgsl` is the code editor's, which its *asset* layout
/// (`Scripting-Asset`) names. Stripping the `-Asset` suffix reads that mapping
/// off the data instead of hard-coding the exception, and it keeps the
/// `scene_layout_names_are_unique` invariant those tables are tested against.
fn kind_in_workspace(kind: renzora_ui::DocTabKind, workspace: &str) -> bool {
    kind.layout_name() == Some(workspace)
        || kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset"))
            == Some(workspace)
}

/// Follow the active document: point [`renzora_ui::EditorContext`] at it and
/// switch the workspace its kind maps to.
///
/// This runs for *every* activation (the switcher, a programmatic open such as
/// double-clicking an asset or the inspector's "edit" button, a close that
/// promotes its neighbour), because it watches the active id rather than any one
/// route. The `EditorContext` half is what makes moving to a second material
/// swap the graph: every asset panel loads from the context's path, so without
/// it the dock switched to the Materials workspace and left the *previous*
/// material in it. `open_asset_tab` sets the context when it opens a document;
/// nothing else did when you moved between two already-open ones.
///
/// The `Local` change-guard means it only fires on a real active-document
/// change, so ribbon navigation while a document is open isn't fought (the
/// scene entities are never touched; this is purely a layout switch).
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_workspace_to_active_doc(
    state: Option<Res<renzora_ui::DocumentTabState>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
    project: Option<Res<renzora::CurrentProject>>,
    mut mru: ResMut<DocTabMru>,
    mut commands: Commands,
    mut last: Local<Option<u64>>,
) {
    let Some(state) = state else { return };
    let active_id = state.active_tab_id();
    if *last == active_id {
        return;
    }
    *last = active_id;
    let Some(tab) = state.active_tab() else { return };

    // Newest last. Dropping ids of closed documents here keeps the stack from
    // growing for a session's worth of opens and closes.
    mru.0.retain(|id| *id != tab.id && state.tabs.iter().any(|t| t.id == *id));
    mru.0.push(tab.id);

    // Asset panels read their file straight off this, so it has to move with the
    // document. Written only when it actually differs: it's a change-detected
    // resource, and several panels reload on any change to it.
    if let Some(mut context) = context {
        let next = renzora_ui::EditorContext::from_tab(tab);
        if *context != next {
            *context = next;
        }
    }

    // The kind's own workspace, or (for a kind naming one that doesn't exist)
    // the workspace its asset layout is derived from, which is the same fallback
    // [`kind_in_workspace`] accepts. Shaders are the case: `Shaders` is not a
    // workspace, but `Scripting-Asset` says the code editor's is where a `.wgsl`
    // belongs.
    let wi = [
        tab.kind.layout_name(),
        tab.kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset")),
    ]
    .into_iter()
    .flatten()
    .find_map(|name| layouts.layouts.iter().position(|(n, _)| n == name));
    if let Some(wi) = wi {
        apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
    }

    // A script or shader has no panel that reads `EditorContext`: the code
    // editor keeps its own list of open files and only ever hears about one
    // through `OpenCodeEditorFile`. Asking again for a file it already holds
    // just focuses that tab, so this is the same move `open_asset_tab` makes,
    // on the route it doesn't cover: moving between two documents already open.
    //
    // Revealing the panel belongs here rather than at the asset browser's
    // double-click, because it has to happen *after* the workspace switch
    // above. Done there, the code editor was added to the layout we were on
    // the way out of.
    match tab.kind {
        renzora_ui::DocTabKind::Script | renzora_ui::DocTabKind::Shader => {
            if let (Some(rel), Some(project)) = (tab.scene_path.as_ref(), project) {
                commands.insert_resource(renzora::core::OpenCodeEditorFile {
                    path: project.resolve_path(rel),
                });
            }
            focus_panel(&mut dock, &mut dirty, "code_editor");
        }
        // A UI template's document *is* a canvas, so returning to it re-selects
        // that canvas and reveals the panel showing it: the same move, one
        // panel over.
        renzora_ui::DocTabKind::Ui => {
            if let (Some(rel), Some(project)) = (tab.scene_path.as_ref(), project) {
                commands.insert_resource(renzora::core::OpenUiTemplateFile {
                    path: project.resolve_path(rel),
                });
            }
            focus_panel(&mut dock, &mut dirty, "ui_canvas");
        }
        _ => {}
    }
}

/// Bring `panel` forward, rebuilding the dock only if that changed the layout.
///
/// It used to flag a rebuild every time. Picking the Scripting workspace off
/// the ribbon rebuilds the dock once, brings the script document forward, and
/// that document change came back through here and rebuilt the whole dock
/// again a frame later with the code editor already the active tab: the
/// double flicker on a workspace switch.
fn focus_panel(dock: &mut Dock, dirty: &mut DockDirty, panel: &str) {
    let before = dock.tree.clone();
    dock.tree.focus_or_add_panel(panel);
    if dock.tree != before {
        dirty.0 = true;
    }
}

/// The other direction: switching workspace brings that workspace's document
/// forward. Pick the Materials workspace off the ribbon and the material you
/// were last editing is the active document again, with its graph loaded,
/// rather than the Materials layout sitting there showing whatever the scene you
/// were in happens to select.
///
/// Nothing happens when the active document already belongs to the workspace
/// being switched to, which is what keeps this from fighting
/// [`sync_workspace_to_active_doc`]: activating a document switches the
/// workspace, this system sees that change, finds the document that caused it
/// already in place, and stops. That check is on the *active document* rather
/// than on the MRU stack deliberately: it holds whichever of the two systems
/// runs first in a frame, where "is the MRU top fresh yet" would not.
///
/// A workspace no open document maps to (Debug, Hub, Animation) leaves the
/// active document alone: there is nothing there to bring forward.
pub(crate) fn sync_active_doc_to_workspace(
    layouts: Res<ShellLayouts>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mru: Res<DocTabMru>,
    mut last: Local<Option<usize>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else { return };
    if *last == Some(layouts.active) {
        return;
    }
    *last = Some(layouts.active);
    let Some((name, _)) = layouts.layouts.get(layouts.active) else {
        return;
    };
    if state
        .active_tab()
        .is_some_and(|t| kind_in_workspace(t.kind, name))
    {
        return;
    }
    // Most recent first; falling back to display order for a workspace you have
    // documents in but have never been to this session (restored ones, say).
    let idx = mru
        .0
        .iter()
        .rev()
        .find_map(|id| {
            state
                .tabs
                .iter()
                .position(|t| t.id == *id && kind_in_workspace(t.kind, name))
        })
        .or_else(|| {
            state
                .tabs
                .iter()
                .position(|t| kind_in_workspace(t.kind, name))
        });
    let Some(idx) = idx else { return };
    // Through `activate_tab` + `TabSwitchRequest` like every other switch, so a
    // scene→scene move still swaps the live scene for the incoming one's buffer.
    if let Some((old_id, new_id)) = state.activate_tab(idx) {
        commands.insert_resource(renzora::TabSwitchRequest {
            old_tab_id: old_id,
            new_tab_id: new_id,
        });
    }
}

/// Follow a renamed or moved asset in the open documents, so a rename from
/// anywhere (the asset browser, a folder move) leaves every open document
/// pointing at the file it actually has open rather than at a dead path.
pub(crate) fn doc_tabs_follow_asset_path(
    trigger: On<renzora::AssetPathChanged>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
) {
    let ev = trigger.event();
    if let Some(mut state) = state {
        for tab in state.tabs.iter_mut() {
            let Some(new_path) = tab.scene_path.as_ref().and_then(|p| ev.rewrite(p)) else {
                continue;
            };
            if let Some(stem) = std::path::Path::new(&new_path)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                tab.name = stem.to_string();
            }
            tab.scene_path = Some(new_path);
        }
    }
    // Asset-mode panels load straight from this path, so it has to move too.
    if let Some(mut context) = context {
        if let renzora_ui::EditorContext::Asset { path, .. } = &mut *context {
            if let Some(new_path) = ev.rewrite(path) {
                *path = new_path;
            }
        }
    }
}

/// Close a document by id and, if it was the active one, fire a
/// [`renzora::TabSwitchRequest`] so the viewport follows to the newly-active
/// document. `close_tab` only moves the active index and never swaps scene
/// content, so without this the old scene would linger under a different
/// active document.
pub(crate) fn close_doc_tab_by_id(
    state: &mut renzora_ui::DocumentTabState,
    id: u64,
    commands: &mut Commands,
) {
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let was_active = state.active_tab == idx;
    // The active document's id before the close, used as `old` for the switch
    // so `handle_tab_switch` despawns the current scene before loading the next.
    let prev_active_id = state.active_tab_id();
    if state.close_tab(idx).is_some() && was_active {
        if let (Some(old), Some(new)) = (prev_active_id, state.active_tab_id()) {
            if old != new {
                commands.insert_resource(renzora::TabSwitchRequest {
                    old_tab_id: old,
                    new_tab_id: new,
                });
            }
        }
    }
}
