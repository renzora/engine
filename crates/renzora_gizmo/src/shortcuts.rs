//! Keyboard shortcuts that act on the selection or the document: delete,
//! deselect, copy/paste/duplicate, the file commands, and the tool-mode keys.
//!
//! These deliberately are NOT gated on the 3D view — Delete on a 2D entity has
//! to work from the Hierarchy too — so the plugin registers them outside the
//! `in_three_view` chain.

use bevy::prelude::*;

use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportState;
use renzora::core::InputFocusState;
use renzora_editor_framework::EditorSelection;

use crate::modal_transform;
use crate::placement;
use crate::types::{GizmoMode, GizmoState};

/// One-shot resource to signal pending modal grab from duplicate-and-move.
#[derive(Resource)]
pub(crate) struct PendingModalGrab;

/// Editor-wide clipboard for Copy/Paste of entities. Stores the source
/// entity ids captured at copy time; paste deep-clones each via
/// `EntityWorldMut::clone_and_spawn`, so all components transfer. Sources
/// that have been despawned between copy and paste are silently skipped.
#[derive(Resource, Default, Clone, Debug)]
pub struct EditorClipboard {
    pub entities: Vec<Entity>,
}

pub(crate) fn handle_selection_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    viewport_state: Res<ViewportState>,
    selection: Res<EditorSelection>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    gizmo_state: Res<GizmoState>,
    modal: Res<modal_transform::ModalTransformState>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if gizmo_state.active_axis.is_some() {
        return;
    }
    if modal.active {
        return;
    }

    // Delete fires from any panel (e.g. selecting in the Hierarchy and
    // pressing Delete without moving the cursor into the viewport). It is
    // suppressed while a panel (e.g. the animation timeline with a keyframe
    // selected) is consuming Delete to remove the keyframe, not the entity.
    if keybindings.just_pressed(EditorAction::Delete, &keyboard) && !input_focus.suppress_entity_delete {
        let entities = selection.get_all();
        if !entities.is_empty() {
            // Faithful subtree-snapshot delete (works for lights, cameras, GLB
            // imports, 2D nodes, groups) — not just default-mesh primitives. The
            // reselect variant moves selection to the nearest survivor afterwards
            // so the viewport doesn't go blank (don't pre-clear here — that's what
            // left it blank).
            commands.queue(move |world: &mut World| {
                renzora_undo::delete_entities_with_undo_reselect(world, &entities);
            });
        }
    }

    if input_focus.pointer_over_ui && !viewport_state.hovered {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.clear();
    }

    if keybindings.just_pressed(EditorAction::CreateNode, &keyboard) {
        commands.insert_resource(renzora::core::CreateNodeRequested);
    }

    // Copy (Ctrl+C) — snapshot the current selection into the clipboard.
    if keybindings.just_pressed(EditorAction::Copy, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                world.insert_resource(EditorClipboard { entities });
            });
        }
    }

    // Paste (Ctrl+V) — clone every entity on the clipboard (filtering out
    // ones that have since been despawned) and select the copies. If the
    // cursor is over the viewport, the copies land on the surface under it,
    // so paste follows the camera/cursor. Otherwise they land at their
    // original world position.
    if keybindings.just_pressed(EditorAction::Paste, &keyboard) {
        commands.queue(move |world: &mut World| {
            let sources = world
                .get_resource::<EditorClipboard>()
                .map(|c| c.entities.clone())
                .unwrap_or_default();
            if sources.is_empty() {
                return;
            }

            let paste_target = placement::paste_target(world);
            duplicate_entities(world, &sources);

            if let Some(target) = paste_target {
                let new_ids = world
                    .get_resource::<EditorSelection>()
                    .map(|s| s.get_all())
                    .unwrap_or_default();
                placement::place_on(world, &new_ids, target);
            }
        });
    }

    // Move selection to cursor (V): the selection lands centred on the
    // surface under the cursor, so something selected in the Hierarchy far
    // out of view can be fetched without flying to it.
    if keybindings.just_pressed(EditorAction::MoveSelectionToCursor, &keyboard) {
        commands.queue(placement::move_selection_to_cursor);
    }

    // Drop to surface (Page Down): each selected entity settles onto
    // whatever is beneath it.
    if keybindings.just_pressed(EditorAction::DropToSurface, &keyboard) {
        commands.queue(placement::drop_selection_to_surface);
    }

    // Duplicate (Ctrl+D)
    if keybindings.just_pressed(EditorAction::Duplicate, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
        }
    }

    // Duplicate & Move (Alt+D) — duplicate then enter grab mode
    if keybindings.just_pressed(EditorAction::DuplicateAndMove, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
            commands.insert_resource(PendingModalGrab);
        }
    }
}

/// Deep-clone each selected entity (all components, via Bevy's
/// `EntityWorldMut::clone_and_spawn`) and replace the selection with the
/// new copies. The suffix " (Copy)" is appended to the `Name` so
/// duplicates are distinguishable in the hierarchy.
fn duplicate_entities(world: &mut World, sources: &[Entity]) {
    let mut new_ids: Vec<Entity> = Vec::with_capacity(sources.len());
    for src in sources {
        let Ok(mut src_mut) = world.get_entity_mut(*src) else {
            continue;
        };
        let new = src_mut.clone_and_spawn();
        // Append " (Copy)" to the cloned entity's Name.
        if let Some(original) = world.get::<Name>(new).map(|n| n.as_str().to_string()) {
            if let Ok(mut ent) = world.get_entity_mut(new) {
                ent.insert(Name::new(format!("{} (Copy)", original)));
            }
        }
        new_ids.push(new);
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.clear();
        for e in &new_ids {
            sel.toggle(*e);
        }
    }
}

/// Handle file & edit keyboard shortcuts (save, open, settings, etc.).
pub(crate) fn handle_file_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<modal_transform::ModalTransformState>,
    pending_grab: Option<Res<PendingModalGrab>>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }

    // Consume pending grab from duplicate-and-move
    if pending_grab.is_some() {
        commands.remove_resource::<PendingModalGrab>();
        modal.pending_grab = true;
    }

    // Save (Ctrl+S)
    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(renzora::core::SaveSceneRequested);
    }

    // Save As (Ctrl+Shift+S)
    if keybindings.just_pressed(EditorAction::SaveSceneAs, &keyboard) {
        commands.insert_resource(renzora::core::SaveAsSceneRequested);
    }

    // Open Scene (Ctrl+O)
    if keybindings.just_pressed(EditorAction::OpenScene, &keyboard) {
        commands.insert_resource(renzora::core::OpenSceneRequested);
    }

    // New Scene (Ctrl+N)
    if keybindings.just_pressed(EditorAction::NewScene, &keyboard) {
        commands.insert_resource(renzora::core::NewSceneRequested);
    }

    // Settings (Ctrl+,)
    if keybindings.just_pressed(EditorAction::OpenSettings, &keyboard) {
        commands.insert_resource(renzora::core::ToggleSettingsRequested);
    }
}

// ── Mode switching ──────────────────────────────────────────────────────────

pub(crate) fn switch_gizmo_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
    viewport_settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    mut mode: ResMut<GizmoMode>,
    mut active_tool: ResMut<renzora_editor_framework::ActiveTool>,
) {
    // Scene mode only, for the same reason `modal_transform_input_system` is:
    // any other mode belongs to whichever plugin drives it, and these keys are
    // not free. `GizmoRotate` defaults to `E`, which is also mesh-edit's
    // extrude — so pressing `E` in Edit mode both extruded the mesh and threw
    // the scene tool over to Rotate, which `enter_edit_mode` then stomped back
    // to `None` on the next frame. `R` (`GizmoScale`) collides with the loop
    // cut's `Ctrl+R` the same way.
    if !matches!(
        viewport_settings.as_deref().map(|s| s.viewport_mode),
        None | Some(renzora::core::viewport_types::ViewportMode::Scene)
    ) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }
    if keybindings.just_pressed(EditorAction::ToolSelect, &keyboard) {
        *mode = GizmoMode::Select;
        *active_tool = renzora_editor_framework::ActiveTool::Select;
    }
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        *mode = GizmoMode::Translate;
        *active_tool = renzora_editor_framework::ActiveTool::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        *mode = GizmoMode::Rotate;
        *active_tool = renzora_editor_framework::ActiveTool::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        *mode = GizmoMode::Scale;
        *active_tool = renzora_editor_framework::ActiveTool::Scale;
    }
}
