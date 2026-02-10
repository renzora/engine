//! Play mode system for testing games in the editor
//!
//! Allows users to test their game from the editor by activating the scene's
//! default camera and hiding the editor UI.

use bevy::prelude::*;
use bevy::camera::RenderTarget;

use avian3d::prelude::*;

use crate::core::{AppState, PlayModeCamera, PlayModeState, PlayState, ViewportCamera};
use crate::viewport::ViewportImage;
use crate::shared::{
    CameraNodeData, CameraRigData, CollisionShapeData, PhysicsBodyData, RuntimePhysics,
    spawn_entity_physics, despawn_physics_components,
};
use crate::{console_info, console_success, console_warn};

pub struct PlayModePlugin;

impl Plugin for PlayModePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_play_mode_input,
                handle_play_mode_transitions,
                handle_physics_transitions,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        );
    }
}

/// Handle keyboard input for play mode (F5 to play, Escape to stop)
fn handle_play_mode_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut play_mode: ResMut<PlayModeState>,
) {
    // F5 to play/resume
    if keyboard.just_pressed(KeyCode::F5) {
        if play_mode.is_editing() {
            play_mode.request_play = true;
        } else if play_mode.is_paused() {
            play_mode.state = PlayState::Playing;
        }
    }

    // F6 to pause
    if keyboard.just_pressed(KeyCode::F6) && play_mode.is_playing() {
        play_mode.state = PlayState::Paused;
    }

    // Escape to stop
    if keyboard.just_pressed(KeyCode::Escape) && play_mode.is_in_play_mode() {
        play_mode.request_stop = true;
    }
}

/// Handle play mode state transitions
fn handle_play_mode_transitions(
    mut commands: Commands,
    mut play_mode: ResMut<PlayModeState>,
    cameras: Query<(Entity, &CameraNodeData, &Transform), Without<CameraRigData>>,
    camera_rigs: Query<(Entity, &CameraRigData, &Transform), Without<CameraNodeData>>,
    play_mode_cameras: Query<Entity, With<PlayModeCamera>>,
    mut editor_camera: Query<&mut Camera, With<ViewportCamera>>,
    viewport_image: Res<ViewportImage>,
    // Physics queries
    physics_entities: Query<
        (Entity, Option<&PhysicsBodyData>, Option<&CollisionShapeData>),
        Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>,
    >,
    runtime_physics_entities: Query<Entity, With<RuntimePhysics>>,
) {
    // Handle request to enter play mode (fullscreen)
    if play_mode.request_play {
        play_mode.request_play = false;
        enter_play_mode(&mut commands, &mut play_mode, &cameras, &camera_rigs, &mut editor_camera, &viewport_image);

        // Spawn physics components for all entities with physics data
        spawn_play_mode_physics(&mut commands, &physics_entities);
    }

    // Handle request to enter scripts-only mode (no camera switch)
    if play_mode.request_scripts_only {
        play_mode.request_scripts_only = false;
        enter_scripts_only_mode(&mut commands, &mut play_mode, &physics_entities);
    }

    // Handle request to exit play mode
    if play_mode.request_stop {
        play_mode.request_stop = false;

        // Check if we're in scripts-only mode (no camera cleanup needed)
        if play_mode.is_scripts_only() {
            exit_scripts_only_mode(&mut commands, &mut play_mode, &runtime_physics_entities);
        } else {
            exit_play_mode(&mut commands, &mut play_mode, &play_mode_cameras, &mut editor_camera);
            // Despawn physics components from all entities
            despawn_play_mode_physics(&mut commands, &runtime_physics_entities);
        }
    }
}

/// Handle physics pause/unpause based on play state
fn handle_physics_transitions(
    play_mode: Res<PlayModeState>,
    mut physics_time: ResMut<Time<Physics>>,
) {
    // Only process if play mode state changed
    if !play_mode.is_changed() {
        return;
    }

    match play_mode.state {
        PlayState::Playing | PlayState::ScriptsOnly => {
            if physics_time.is_paused() {
                physics_time.unpause();
                info!("Physics unpaused for play mode");
            }
        }
        PlayState::Paused | PlayState::ScriptsPaused => {
            if !physics_time.is_paused() {
                physics_time.pause();
                info!("Physics paused");
            }
        }
        PlayState::Editing => {
            if !physics_time.is_paused() {
                physics_time.pause();
                info!("Physics paused (editing mode)");
            }
        }
    }
}

/// Enter scripts-only mode: run scripts without switching camera
fn enter_scripts_only_mode(
    commands: &mut Commands,
    play_mode: &mut PlayModeState,
    physics_entities: &Query<
        (Entity, Option<&PhysicsBodyData>, Option<&CollisionShapeData>),
        Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>,
    >,
) {
    console_info!("Scripts", "Running scripts in editor...");

    // Spawn physics components
    spawn_play_mode_physics(commands, physics_entities);

    play_mode.state = PlayState::ScriptsOnly;
    console_success!("Scripts", "Scripts active - editor camera retained");
}

/// Exit scripts-only mode
fn exit_scripts_only_mode(
    commands: &mut Commands,
    play_mode: &mut PlayModeState,
    runtime_physics_entities: &Query<Entity, With<RuntimePhysics>>,
) {
    // Despawn physics components
    despawn_play_mode_physics(commands, runtime_physics_entities);

    play_mode.state = PlayState::Editing;
    console_info!("Scripts", "Scripts stopped");
}

/// Spawn physics components for all entities with PhysicsBodyData or CollisionShapeData
fn spawn_play_mode_physics(
    commands: &mut Commands,
    physics_entities: &Query<
        (Entity, Option<&PhysicsBodyData>, Option<&CollisionShapeData>),
        Or<(With<PhysicsBodyData>, With<CollisionShapeData>)>,
    >,
) {
    let mut count = 0;
    for (entity, body_data, shape_data) in physics_entities.iter() {
        spawn_entity_physics(commands, entity, body_data, shape_data);
        count += 1;
    }

    if count > 0 {
        console_info!("Physics", "Spawned physics for {} entities", count);
    }
}

/// Despawn physics components from all entities marked with RuntimePhysics
fn despawn_play_mode_physics(
    commands: &mut Commands,
    runtime_physics_entities: &Query<Entity, With<RuntimePhysics>>,
) {
    let mut count = 0;
    for entity in runtime_physics_entities.iter() {
        despawn_physics_components(commands, entity);
        commands.entity(entity).remove::<RuntimePhysics>();
        count += 1;
    }

    if count > 0 {
        console_info!("Physics", "Cleaned up physics from {} entities", count);
    }
}

/// Enter play mode: activate game camera, hide editor camera
fn enter_play_mode(
    commands: &mut Commands,
    play_mode: &mut PlayModeState,
    cameras: &Query<(Entity, &CameraNodeData, &Transform), Without<CameraRigData>>,
    camera_rigs: &Query<(Entity, &CameraRigData, &Transform), Without<CameraNodeData>>,
    editor_camera: &mut Query<&mut Camera, With<ViewportCamera>>,
    viewport_image: &ViewportImage,
) {
    info!("Entering play mode");
    console_info!("Play Mode", "Starting play mode...");

    // Find the default camera (is_default_camera=true) - check both cameras and rigs
    // First check regular cameras
    let default_camera = cameras
        .iter()
        .find(|(_, data, _)| data.is_default_camera);

    // Then check camera rigs
    let default_rig = camera_rigs
        .iter()
        .find(|(_, data, _)| data.is_default_camera);

    // Prefer the one that's marked as default, or fall back to first available
    if let Some((entity, data, transform)) = default_camera {
        info!(
            "Activating game camera {:?} at {:?} with fov {}",
            entity, transform.translation, data.fov
        );

        // Add Camera3d component to the game camera entity
        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1, // Render on top of editor camera
                ..default()
            },
            RenderTarget::Image(viewport_image.0.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: data.fov.to_radians(),
                ..default()
            }),
            PlayModeCamera,
        ));

        play_mode.active_game_camera = Some(entity);
        console_success!("Play Mode", "Game camera activated (FOV: {:.0}°)", data.fov);

        // Disable the editor camera
        for mut camera in editor_camera.iter_mut() {
            camera.is_active = false;
        }
    } else if let Some((entity, rig_data, transform)) = default_rig {
        info!(
            "Activating camera rig {:?} at {:?} with fov {}",
            entity, transform.translation, rig_data.fov
        );

        // Add Camera3d component to the camera rig entity
        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1, // Render on top of editor camera
                ..default()
            },
            RenderTarget::Image(viewport_image.0.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: rig_data.fov.to_radians(),
                ..default()
            }),
            PlayModeCamera,
        ));

        play_mode.active_game_camera = Some(entity);
        console_success!("Play Mode", "Camera rig activated (FOV: {:.0}°)", rig_data.fov);

        // Disable the editor camera
        for mut camera in editor_camera.iter_mut() {
            camera.is_active = false;
        }
    } else if let Some((entity, data, transform)) = cameras.iter().next() {
        // Fall back to first regular camera
        info!(
            "Activating first camera {:?} at {:?} with fov {}",
            entity, transform.translation, data.fov
        );

        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1,
                ..default()
            },
            RenderTarget::Image(viewport_image.0.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: data.fov.to_radians(),
                ..default()
            }),
            PlayModeCamera,
        ));

        play_mode.active_game_camera = Some(entity);
        console_success!("Play Mode", "Game camera activated (FOV: {:.0}°)", data.fov);

        for mut camera in editor_camera.iter_mut() {
            camera.is_active = false;
        }
    } else if let Some((entity, rig_data, transform)) = camera_rigs.iter().next() {
        // Fall back to first camera rig
        info!(
            "Activating first camera rig {:?} at {:?} with fov {}",
            entity, transform.translation, rig_data.fov
        );

        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1,
                ..default()
            },
            RenderTarget::Image(viewport_image.0.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: rig_data.fov.to_radians(),
                ..default()
            }),
            PlayModeCamera,
        ));

        play_mode.active_game_camera = Some(entity);
        console_success!("Play Mode", "Camera rig activated (FOV: {:.0}°)", rig_data.fov);

        for mut camera in editor_camera.iter_mut() {
            camera.is_active = false;
        }
    } else {
        warn!("No camera found in scene - game will render nothing");
        console_warn!("Play Mode", "No camera in scene - nothing will render");
        // Still enter play mode, just with no camera
    }

    play_mode.state = PlayState::Playing;
}

/// Exit play mode: remove game camera, restore editor camera
fn exit_play_mode(
    commands: &mut Commands,
    play_mode: &mut PlayModeState,
    play_mode_cameras: &Query<Entity, With<PlayModeCamera>>,
    editor_camera: &mut Query<&mut Camera, With<ViewportCamera>>,
) {
    info!("Exiting play mode");

    // Remove Camera3d components from all play mode cameras
    for entity in play_mode_cameras.iter() {
        commands
            .entity(entity)
            .remove::<Camera3d>()
            .remove::<Camera>()
            .remove::<Projection>()
            .remove::<RenderTarget>()
            .remove::<PlayModeCamera>();
    }

    play_mode.active_game_camera = None;

    // Re-enable the editor camera
    for mut camera in editor_camera.iter_mut() {
        camera.is_active = true;
    }

    play_mode.state = PlayState::Editing;
    console_info!("Play Mode", "Stopped - returned to editor");
}
