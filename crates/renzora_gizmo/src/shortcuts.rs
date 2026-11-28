//! Keyboard shortcuts that act on the selection or the document: delete,
//! deselect, copy/paste/duplicate, the file commands, and the tool-mode keys.
//!
//! These deliberately are NOT gated on the 3D view — Delete on a 2D entity has
//! to work from the Hierarchy too — so the plugin registers them outside the
//! `in_three_view` chain.

use bevy::prelude::*;

use renzora_shell_api::keybindings::{EditorAction, KeyBindings};
use renzora_viewport_api::ViewportState;
use renzora_project_api::InputFocusState;
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
        commands.insert_resource(renzora_project_api::CreateNodeRequested);
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

/// Deep-clone each selected entity and everything under it, and replace the
/// selection with the new copies. The suffix " (Copy)" is appended to the
/// `Name` so duplicates are distinguishable in the hierarchy.
///
/// The children come too (`linked_cloning`). A plain `clone_and_spawn` copied
/// the root alone, so duplicating a prefab instance gave an empty instance:
/// its contents are children, and the editor only expands an instance when
/// one is added, never when it is cloned.
///
/// Left out of the copy:
/// - a physics backend's own components. Avian's `RigidBodyColliders` is a
///   linked relationship that lists a body as its own collider, so following it
///   cloned the entity into itself without end; the copy carries the engine's
///   physics data and the backend builds its parts afresh.
/// - `HideInHierarchy` descendants: runtime parts such as a particle effect's
///   renderer children, which the copy's own components rebuild. Cloned, they
///   became visible, named rows drawing a second, frozen copy of the effect.
fn duplicate_entities(world: &mut World, sources: &[Entity]) {
    let skip: Vec<bevy::ecs::component::ComponentId> = match world
        .get_resource::<renzora_physics_api::physics::PhysicsBackends>()
    {
        Some(backends) => {
            let registry = world.resource::<AppTypeRegistry>().read();
            registry
                .iter()
                .filter(|r| backends.is_runtime_type(r.type_info().type_path()))
                .filter_map(|r| world.components().get_id(r.type_id()))
                .collect()
        }
        None => Vec::new(),
    };
    let mut new_ids: Vec<Entity> = Vec::with_capacity(sources.len());
    for src in sources {
        let Ok(mut src_mut) = world.get_entity_mut(*src) else {
            continue;
        };
        let skip = skip.clone();
        let new = src_mut.clone_and_spawn_with_opt_out(move |builder| {
            builder.linked_cloning(true).deny_by_ids(skip);
        });
        prune_hidden_descendants(world, new);
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

/// Despawn every `HideInHierarchy` entity under `root` (see
/// [`duplicate_entities`]).
fn prune_hidden_descendants(world: &mut World, root: Entity) {
    let mut stack = vec![root];
    let mut doomed = Vec::new();
    while let Some(e) = stack.pop() {
        let Some(children) = world.get::<Children>(e) else { continue };
        for child in children.iter() {
            if world.get::<renzora_scene_api::HideInHierarchy>(child).is_some() {
                doomed.push(child);
            } else {
                stack.push(child);
            }
        }
    }
    for e in doomed {
        if let Ok(ent) = world.get_entity_mut(e) {
            ent.despawn();
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
        commands.insert_resource(renzora_project_api::SaveSceneRequested);
    }

    // Save As (Ctrl+Shift+S)
    if keybindings.just_pressed(EditorAction::SaveSceneAs, &keyboard) {
        commands.insert_resource(renzora_project_api::SaveAsSceneRequested);
    }

    // Open Scene (Ctrl+O)
    if keybindings.just_pressed(EditorAction::OpenScene, &keyboard) {
        commands.insert_resource(renzora_project_api::OpenSceneRequested);
    }

    // New Scene (Ctrl+N)
    if keybindings.just_pressed(EditorAction::NewScene, &keyboard) {
        commands.insert_resource(renzora_project_api::NewSceneRequested);
    }

    // Settings (Ctrl+,)
    if keybindings.just_pressed(EditorAction::OpenSettings, &keyboard) {
        commands.insert_resource(renzora_project_api::ToggleSettingsRequested);
    }
}

// ── Mode switching ──────────────────────────────────────────────────────────

pub(crate) fn switch_gizmo_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
    viewport_settings: Option<Res<renzora_viewport_api::ViewportSettings>>,
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
        None | Some(renzora_viewport_api::ViewportMode::Scene)
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
