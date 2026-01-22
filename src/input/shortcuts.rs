use bevy::prelude::*;

use crate::core::{KeyBindings, EditorAction, SelectionState};
use crate::gizmo::{GizmoMode, GizmoState};

pub fn handle_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    mut selection: ResMut<SelectionState>,
    mut gizmo: ResMut<GizmoState>,
    mut commands: Commands,
) {
    // Don't process keybindings while rebinding
    if keybindings.rebinding.is_some() {
        return;
    }

    if keybindings.just_pressed(EditorAction::Delete, &keyboard) {
        if let Some(entity) = selection.selected_entity {
            commands.entity(entity).despawn();
            selection.selected_entity = None;
        }
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.selected_entity = None;
    }

    // Gizmo mode hotkeys
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        gizmo.mode = GizmoMode::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        gizmo.mode = GizmoMode::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        gizmo.mode = GizmoMode::Scale;
    }
}
