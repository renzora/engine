//! Runtime camera activation
//!
//! Handles activating the default game camera in the runtime.

use bevy::prelude::*;

use super::shared::CameraNodeData;

/// Plugin to handle runtime camera activation
pub struct RuntimeCameraPlugin;

impl Plugin for RuntimeCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, activate_game_camera.after(super::loader::load_main_scene));
    }
}

/// Marker component for the active game camera
#[derive(Component)]
pub struct GameCamera;

/// Activate the default game camera
/// Priority: is_default_camera=true > first camera > none
fn activate_game_camera(
    mut commands: Commands,
    cameras: Query<(Entity, &CameraNodeData, &Transform)>,
) {
    // Find the default camera (is_default_camera=true) or use first camera
    let camera = cameras
        .iter()
        .find(|(_, data, _)| data.is_default_camera)
        .or_else(|| cameras.iter().next());

    if let Some((entity, data, transform)) = camera {
        info!("Activating game camera {:?} with fov {}", entity, data.fov);

        // Insert Camera3d component to make this the active camera
        commands.entity(entity).insert((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: data.fov.to_radians(),
                ..default()
            }),
            GameCamera,
        ));

        info!("Game camera activated at position {:?}", transform.translation);
    } else {
        warn!("No camera found in scene - game will render nothing");
    }
}
