//! Editor UI camera setup

use bevy::prelude::*;

/// Spawns the 2D camera used for the egui UI overlay.
pub fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            ..default()
        },
    ));
}
