use bevy::prelude::*;

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::core::{KeyBindings, EditorAction, InputFocusState, SelectionState};
use crate::gizmo::{EditorTool, GizmoMode, GizmoState, ModalTransformState};

pub fn handle_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut selection: ResMut<SelectionState>,
    mut gizmo: ResMut<GizmoState>,
    mut command_history: ResMut<CommandHistory>,
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

    if keybindings.just_pressed(EditorAction::Delete, &keyboard) {
        // Delete all selected entities
        let entities_to_delete: Vec<_> = selection.get_all_selected();
        for entity in entities_to_delete {
            // Queue delete command for undo support
            queue_command(&mut command_history, Box::new(DeleteEntityCommand::new(entity)));
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
