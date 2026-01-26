//! Play mode system for testing games in the editor
//!
//! Allows users to test their game from the editor by activating the scene's
//! default camera and hiding the editor UI.

use bevy::prelude::*;

use crate::core::{AppState, PlayModeCamera, PlayModeState, PlayState, ViewportCamera};
use crate::shared::CameraNodeData;
use crate::shared::CameraRigData;
use crate::{console_info, console_success, console_warn};

pub struct PlayModePlugin;

impl Plugin for PlayModePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_play_mode_input,
                handle_play_mode_transitions,
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
) {
    // Handle request to enter play mode
    if play_mode.request_play {
        play_mode.request_play = false;
        enter_play_mode(&mut commands, &mut play_mode, &cameras, &camera_rigs, &mut editor_camera);
    }

    // Handle request to exit play mode
    if play_mode.request_stop {
        play_mode.request_stop = false;
        exit_play_mode(&mut commands, &mut play_mode, &play_mode_cameras, &mut editor_camera);
    }
}

/// Enter play mode: activate game camera, hide editor camera
fn enter_play_mode(
    commands: &mut Commands,
    play_mode: &mut PlayModeState,
    cameras: &Query<(Entity, &CameraNodeData, &Transform), Without<CameraRigData>>,
    camera_rigs: &Query<(Entity, &CameraRigData, &Transform), Without<CameraNodeData>>,
    editor_camera: &mut Query<&mut Camera, With<ViewportCamera>>,
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
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1, // Render on top of editor camera
                ..default()
            },
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
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1, // Render on top of editor camera
                ..default()
            },
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
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1,
                ..default()
            },
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
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
                order: 1,
                ..default()
            },
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
