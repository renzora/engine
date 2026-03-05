//! Editor camera setup

use bevy::prelude::*;

/// Spawns the default 2D editor camera on startup.
pub fn spawn_editor_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
