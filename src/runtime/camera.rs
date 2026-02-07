//! Runtime camera activation
//!
//! Handles activating the default game camera in the runtime.
//! Supports CameraNodeData (3D), CameraRigData (third-person), and Camera2DData.

use bevy::prelude::*;

use crate::shared::{Camera2DData, CameraNodeData, CameraRigData};

/// Plugin to handle runtime camera activation
pub struct RuntimeCameraPlugin;

impl Plugin for RuntimeCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeCameraState>()
            // Run camera activation in Update to handle async scene loading
            .add_systems(Update, (
                activate_cameras_for_loaded_entities,
                update_camera_rig,
            ));
    }
}

/// Marker component for the active game camera
#[derive(Component)]
pub struct GameCamera;

/// Marker component for cameras that have been processed
#[derive(Component)]
pub struct RuntimeCamera;

/// Resource to track if we've found a default camera
#[derive(Resource, Default)]
pub struct RuntimeCameraState {
    pub has_active_camera: bool,
}

/// Activate cameras for entities that have camera data components
/// This runs every frame to catch async-loaded scene entities
fn activate_cameras_for_loaded_entities(
    mut commands: Commands,
    mut camera_state: ResMut<RuntimeCameraState>,
    // Query for 3D cameras without RuntimeCamera marker
    new_cameras: Query<
        (Entity, &CameraNodeData, &Transform),
        (Without<RuntimeCamera>, Without<CameraRigData>),
    >,
    // Query for camera rigs without RuntimeCamera marker
    new_camera_rigs: Query<
        (Entity, &CameraRigData, &Transform),
        (Without<RuntimeCamera>, Without<CameraNodeData>),
    >,
    // Query for 2D cameras without RuntimeCamera marker
    new_2d_cameras: Query<
        (Entity, &Camera2DData, &Transform),
        Without<RuntimeCamera>,
    >,
) {
    // Skip if we already have an active camera
    if camera_state.has_active_camera {
        // Still need to mark new cameras so they don't get re-processed
        for (entity, _, _) in new_cameras.iter() {
            commands.entity(entity).insert(RuntimeCamera);
        }
        for (entity, _, _) in new_camera_rigs.iter() {
            commands.entity(entity).insert(RuntimeCamera);
        }
        for (entity, _, _) in new_2d_cameras.iter() {
            commands.entity(entity).insert(RuntimeCamera);
        }
        return;
    }

    // Try to find a default 3D camera
    let default_camera = new_cameras
        .iter()
        .find(|(_, data, _)| data.is_default_camera);

    // Try to find a default camera rig
    let default_rig = new_camera_rigs
        .iter()
        .find(|(_, data, _)| data.is_default_camera);

    // Try to find a default 2D camera
    let default_2d_camera = new_2d_cameras
        .iter()
        .find(|(_, data, _)| data.is_default_camera);

    // Priority: default camera > default rig > default 2D > first available
    if let Some((entity, data, transform)) = default_camera {
        activate_3d_camera(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated default 3D camera {:?}", entity);
    } else if let Some((entity, data, transform)) = default_rig {
        activate_camera_rig(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated default camera rig {:?}", entity);
    } else if let Some((entity, data, transform)) = default_2d_camera {
        activate_2d_camera(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated default 2D camera {:?}", entity);
    } else if let Some((entity, data, transform)) = new_cameras.iter().next() {
        // Fall back to first 3D camera
        activate_3d_camera(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated first available 3D camera {:?}", entity);
    } else if let Some((entity, data, transform)) = new_camera_rigs.iter().next() {
        // Fall back to first camera rig
        activate_camera_rig(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated first available camera rig {:?}", entity);
    } else if let Some((entity, data, transform)) = new_2d_cameras.iter().next() {
        // Fall back to first 2D camera
        activate_2d_camera(&mut commands, entity, data, transform);
        camera_state.has_active_camera = true;
        info!("Activated first available 2D camera {:?}", entity);
    }

    // Mark all cameras as processed
    for (entity, _, _) in new_cameras.iter() {
        commands.entity(entity).insert(RuntimeCamera);
    }
    for (entity, _, _) in new_camera_rigs.iter() {
        commands.entity(entity).insert(RuntimeCamera);
    }
    for (entity, _, _) in new_2d_cameras.iter() {
        commands.entity(entity).insert(RuntimeCamera);
    }
}

/// Activate a 3D perspective camera
fn activate_3d_camera(
    commands: &mut Commands,
    entity: Entity,
    data: &CameraNodeData,
    transform: &Transform,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        Msaa::Off,
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
    info!("3D camera activated at position {:?} with FOV {}", transform.translation, data.fov);
}

/// Activate a camera rig (third-person camera)
fn activate_camera_rig(
    commands: &mut Commands,
    entity: Entity,
    data: &CameraRigData,
    transform: &Transform,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: data.fov.to_radians(),
            ..default()
        }),
        GameCamera,
        CameraRigState {
            target_entity: None,
            target_position: transform.translation + Vec3::new(0.0, 0.0, data.distance),
        },
    ));
    info!(
        "Camera rig activated at position {:?} with distance {} height {}",
        transform.translation, data.distance, data.height
    );
}

/// Activate a 2D orthographic camera
fn activate_2d_camera(
    commands: &mut Commands,
    entity: Entity,
    data: &Camera2DData,
    transform: &Transform,
) {
    // Camera2d marker and projection wrapped in Projection enum
    commands.entity(entity).insert((
        Camera2d,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0 / data.zoom,
            ..OrthographicProjection::default_2d()
        }),
        GameCamera,
    ));
    info!("2D camera activated at position {:?} with zoom {}", transform.translation, data.zoom);
}

/// Runtime state for camera rigs
#[derive(Component)]
pub struct CameraRigState {
    /// Entity to follow (if any)
    pub target_entity: Option<Entity>,
    /// Current target position (for smoothing)
    pub target_position: Vec3,
}

/// Update camera rig position to follow target
fn update_camera_rig(
    time: Res<Time>,
    mut camera_rigs: Query<(&CameraRigData, &mut Transform, &mut CameraRigState), With<GameCamera>>,
    targets: Query<&Transform, Without<GameCamera>>,
) {
    let delta = time.delta_secs();

    for (data, mut transform, mut state) in camera_rigs.iter_mut() {
        // Get target position
        let target_pos = if let Some(target_entity) = state.target_entity {
            if let Ok(target_transform) = targets.get(target_entity) {
                target_transform.translation
            } else {
                state.target_position
            }
        } else {
            state.target_position
        };

        // Smoothly interpolate to target position
        if data.follow_smoothing > 0.0 {
            let t = (data.follow_smoothing * delta).min(1.0);
            state.target_position = state.target_position.lerp(target_pos, t);
        } else {
            state.target_position = target_pos;
        }

        // Calculate camera position based on rig settings
        let offset = Vec3::new(
            data.horizontal_offset,
            data.height,
            data.distance,
        );

        // Apply offset in local space (behind and above target)
        let target_rotation = Quat::from_rotation_y(0.0); // Could be based on target's rotation
        let rotated_offset = target_rotation * offset;
        let desired_position = state.target_position + rotated_offset;

        // Smoothly move camera
        if data.follow_smoothing > 0.0 {
            let t = (data.follow_smoothing * delta).min(1.0);
            transform.translation = transform.translation.lerp(desired_position, t);
        } else {
            transform.translation = desired_position;
        }

        // Look at target
        let look_target = state.target_position + Vec3::new(0.0, data.height * 0.5, 0.0);
        if data.look_smoothing > 0.0 {
            let desired_rotation = transform.looking_at(look_target, Vec3::Y).rotation;
            let t = (data.look_smoothing * delta).min(1.0);
            transform.rotation = transform.rotation.slerp(desired_rotation, t);
        } else {
            transform.look_at(look_target, Vec3::Y);
        }
    }
}
