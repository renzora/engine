//! Editor UI camera setup

use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;

/// Marker for the editor's egui UI camera so play mode can disable it.
#[derive(Component)]
pub struct EditorUiCamera;

/// Spawns the UI camera used for the egui overlay.
///
/// Explicitly inserts `PrimaryEguiContext` so bevy_egui renders on THIS camera
/// (not the 3D editor camera which gets redirected to an offscreen render target).
/// bevy_egui auto-attaches to the first camera spawned, which would be the editor
/// camera — but that one renders offscreen, so egui would never appear on screen.
pub fn spawn_ui_camera(mut commands: Commands) {
    bevy::log::info!("[editor] Spawning UI camera with PrimaryEguiContext");
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            ..default()
        },
        PrimaryEguiContext,
        EditorUiCamera,
        // Make this the default target for bevy_ui roots that don't set their
        // own `UiTargetCamera`. The bevy_ui editor shell (`renzora_shell`)
        // renders onto this existing camera so we don't add a second active
        // window camera (which trips bevy_pbr's atmosphere-probe extraction).
        bevy::ui::IsDefaultUiCamera,
    ));
}
