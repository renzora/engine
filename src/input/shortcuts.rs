use bevy::prelude::*;

use crate::commands::{CommandHistory, DeleteEntityCommand, DuplicateEntityCommand, queue_command};
use crate::core::{KeyBindings, EditorAction, InputFocusState, SelectionState, OrbitCameraState, EditorSettings, PlayModeState};
use crate::gizmo::{EditorTool, GizmoMode, GizmoState, ModalTransformState};

pub fn handle_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut selection: ResMut<SelectionState>,
    mut gizmo: ResMut<GizmoState>,
    mut command_history: ResMut<CommandHistory>,
    mut modal: ResMut<ModalTransformState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    // Don't process shortcuts while modal transform is active
    if modal.active {
        return;
    }

    // Don't process shortcuts when a text input is focused
    if input_focus.egui_wants_keyboard {
        return;
    }

    if keybindings.just_pressed(EditorAction::Delete, &keyboard) {
        // Delete all selected entities
        let entities_to_delete: Vec<_> = selection.get_all_selected();
        for entity in entities_to_delete {
            // Queue delete command for undo support
            queue_command(&mut command_history, Box::new(DeleteEntityCommand::new(entity)));
        }
    }

    if keybindings.just_pressed(EditorAction::Duplicate, &keyboard) {
        // Duplicate all selected entities
        let entities_to_duplicate: Vec<_> = selection.get_all_selected();
        for entity in entities_to_duplicate {
            queue_command(&mut command_history, Box::new(DuplicateEntityCommand::new(entity)));
        }
    }

    if keybindings.just_pressed(EditorAction::DuplicateAndMove, &keyboard) {
        // Duplicate all selected entities and immediately enter grab mode
        let entities_to_duplicate: Vec<_> = selection.get_all_selected();
        let has_entities = !entities_to_duplicate.is_empty();
        for entity in entities_to_duplicate {
            queue_command(&mut command_history, Box::new(DuplicateEntityCommand::new(entity)));
        }
        // Set pending grab to trigger grab mode after duplication
        if has_entities {
            modal.pending_grab = true;
        }
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.clear();
    }

    // Tool mode hotkeys
    if keybindings.just_pressed(EditorAction::ToolSelect, &keyboard) {
        gizmo.tool = EditorTool::Select;
    }
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        gizmo.tool = EditorTool::Transform;
        gizmo.mode = GizmoMode::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        gizmo.tool = EditorTool::Transform;
        gizmo.mode = GizmoMode::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        gizmo.tool = EditorTool::Transform;
        gizmo.mode = GizmoMode::Scale;
    }
}

/// Handle view angle keyboard shortcuts
pub fn handle_view_angles(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut orbit: ResMut<OrbitCameraState>,
    modal: Res<ModalTransformState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    // Don't process shortcuts while modal transform is active
    if modal.active {
        return;
    }

    // Don't process shortcuts when a text input is focused
    if input_focus.egui_wants_keyboard {
        return;
    }

    // View angle shortcuts - yaw/pitch values match ViewAngle enum in viewport.rs
    if keybindings.just_pressed(EditorAction::ViewFront, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewBack, &keyboard) {
        orbit.yaw = std::f32::consts::PI;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewLeft, &keyboard) {
        orbit.yaw = -std::f32::consts::FRAC_PI_2;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewRight, &keyboard) {
        orbit.yaw = std::f32::consts::FRAC_PI_2;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewTop, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = std::f32::consts::FRAC_PI_2;
    }
    if keybindings.just_pressed(EditorAction::ViewBottom, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = -std::f32::consts::FRAC_PI_2;
    }

    // Toggle projection mode
    if keybindings.just_pressed(EditorAction::ToggleProjection, &keyboard) {
        orbit.projection_mode = orbit.projection_mode.toggle();
    }
}

/// Handle render/view toggle shortcuts
pub fn handle_view_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut settings: ResMut<EditorSettings>,
    modal: Res<ModalTransformState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    // Don't process shortcuts while modal transform is active
    if modal.active {
        return;
    }

    // Don't process shortcuts when a text input is focused
    if input_focus.egui_wants_keyboard {
        return;
    }

    // Toggle wireframe
    if keybindings.just_pressed(EditorAction::ToggleWireframe, &keyboard) {
        settings.render_toggles.wireframe = !settings.render_toggles.wireframe;
    }

    // Toggle lighting (unlit mode)
    if keybindings.just_pressed(EditorAction::ToggleLighting, &keyboard) {
        settings.render_toggles.lighting = !settings.render_toggles.lighting;
    }

    // Toggle grid
    if keybindings.just_pressed(EditorAction::ToggleGrid, &keyboard) {
        settings.show_grid = !settings.show_grid;
    }
}

/// Handle play mode shortcut
pub fn handle_play_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut play_mode: ResMut<PlayModeState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    // Don't process shortcuts when a text input is focused
    if input_focus.egui_wants_keyboard {
        return;
    }

    // Play/Stop toggle (F5)
    if keybindings.just_pressed(EditorAction::PlayStop, &keyboard) {
        if play_mode.is_in_play_mode() {
            play_mode.request_stop = true;
        } else {
            play_mode.request_play = true;
        }
    }

    // Scripts only mode (Shift+F5)
    if keybindings.just_pressed(EditorAction::PlayScriptsOnly, &keyboard) {
        if play_mode.is_in_play_mode() {
            play_mode.request_stop = true;
        } else {
            play_mode.request_scripts_only = true;
        }
    }
}
