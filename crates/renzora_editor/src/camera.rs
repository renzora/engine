//! Editor UI camera setup

use bevy::prelude::*;

/// Marker for the editor's UI camera so play mode can disable it.
#[derive(Component)]
pub struct EditorUiCamera;

/// Spawns the 2D camera the bevy_ui editor shell renders onto.
pub fn spawn_ui_camera(mut commands: Commands) {
    bevy::log::info!("[editor] Spawning UI camera");
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            ..default()
        },
        EditorUiCamera,
        // Make this the default target for bevy_ui roots that don't set their
        // own `UiTargetCamera`. The bevy_ui editor shell (`renzora_shell`)
        // renders onto this existing camera so we don't add a second active
        // window camera (which trips bevy_pbr's atmosphere-probe extraction).
        bevy::ui::IsDefaultUiCamera,
    ));
}
