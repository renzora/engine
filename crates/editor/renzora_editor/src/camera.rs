//! Editor UI camera setup

use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;

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
    ));
}
