use bevy::prelude::*;

use crate::core::{EditorState, GizmoMode, KeyBindings, EditorAction};

pub fn handle_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut editor_state: ResMut<EditorState>,
    mut commands: Commands,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    if keybindings.just_pressed(EditorAction::Delete, &keyboard) {
        if let Some(entity) = editor_state.selected_entity {
            commands.entity(entity).despawn();
            editor_state.selected_entity = None;
        }
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        editor_state.selected_entity = None;
    }

    // Gizmo mode hotkeys
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        editor_state.gizmo_mode = GizmoMode::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        editor_state.gizmo_mode = GizmoMode::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        editor_state.gizmo_mode = GizmoMode::Scale;
    }
}
