//! Camera effects system for scripts
//!
//! Processes camera commands and applies effects like screen shake, follow, and zoom.

use bevy::prelude::*;
use crate::core::{PlayModeCamera, PlayModeState};
use crate::scripting::resources::{
    CameraCommand, CameraCommandQueue, ScriptCameraState, ScreenShakeState,
};

/// System to process queued camera commands
pub fn process_camera_commands(
    mut queue: ResMut<CameraCommandQueue>,
    mut camera_state: ResMut<ScriptCameraState>,
) {
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            CameraCommand::SetTarget { position } => {
                camera_state.look_target = Some(position);
            }

            CameraCommand::SetZoom { zoom } => {
                camera_state.zoom = zoom.max(0.1); // Prevent zero/negative zoom
            }

            CameraCommand::ScreenShake { intensity, duration } => {
                // Start new shake or add to existing
                if let Some(ref mut shake) = camera_state.shake {
                    // Combine shakes - take max intensity, extend duration
                    shake.intensity = shake.intensity.max(intensity);
                    shake.initial_intensity = shake.initial_intensity.max(intensity);
                    shake.remaining = shake.remaining.max(duration);
                } else {
                    camera_state.shake = Some(ScreenShakeState::new(intensity, duration));
                }
            }

            CameraCommand::FollowEntity { entity, offset, smoothing } => {
                camera_state.follow_entity = Some(entity);
                camera_state.follow_offset = offset;
                camera_state.follow_smoothing = smoothing.clamp(0.01, 1.0);
            }

            CameraCommand::StopFollow => {
                camera_state.follow_entity = None;
            }
        }
    }
}

/// System to apply camera effects to the play mode camera
pub fn apply_camera_effects(
    time: Res<Time>,
    play_mode: Res<PlayModeState>,
    mut camera_state: ResMut<ScriptCameraState>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<PlayModeCamera>>,
    transforms: Query<&Transform, Without<PlayModeCamera>>,
) {
    // Only apply during play mode
    if !play_mode.is_playing() {
        return;
    }

    let delta = time.delta_secs();

    let has_effects = camera_state.follow_entity.is_some()
        || camera_state.look_target.is_some()
        || camera_state.shake.is_some();

    for (mut transform, mut projection) in camera_query.iter_mut() {
        if has_effects {
            // Store original transform if not already stored
            if camera_state.original_transform.is_none() {
                camera_state.original_transform = Some(*transform);
            }

            let base_transform = camera_state
                .original_transform
                .unwrap_or(*transform);

            // Start with base transform
            let mut final_position = base_transform.translation;
            let mut final_rotation = base_transform.rotation;

            // Apply follow entity
            if let Some(follow_entity) = camera_state.follow_entity {
                if let Ok(target_transform) = transforms.get(follow_entity) {
                    let target_pos = target_transform.translation + camera_state.follow_offset;

                    // Smooth interpolation
                    let t = 1.0 - (-delta / camera_state.follow_smoothing).exp();
                    final_position = transform.translation.lerp(target_pos, t);

                    // Update stored original to track the smooth position
                    if let Some(ref mut orig) = camera_state.original_transform {
                        orig.translation = final_position;
                    }
                }
            }

            // Apply look target
            if let Some(target) = camera_state.look_target {
                let direction = target - final_position;
                if direction.length_squared() > 0.001 {
                    final_rotation = Transform::from_translation(final_position)
                        .looking_at(target, Vec3::Y)
                        .rotation;
                }
            }

            // Apply screen shake
            if let Some(ref mut shake) = camera_state.shake {
                let shake_offset = shake.update(delta);
                final_position += shake_offset;

                // Remove shake when done
                if !shake.is_active() {
                    camera_state.shake = None;
                }
            }

            // Apply final transform
            transform.translation = final_position;
            transform.rotation = final_rotation;
        } else {
            // No active effects — clear stored transform so it stays in sync
            // with any script-driven movement when effects next become active.
            camera_state.original_transform = None;
        }

        // Apply zoom to projection (always)
        if let Projection::Perspective(ref mut persp) = *projection {
            // Zoom affects FOV - higher zoom = lower FOV
            let base_fov = 45.0_f32.to_radians(); // Default FOV
            persp.fov = base_fov / camera_state.zoom;
        }
    }
}

/// System to reset camera state when exiting play mode
pub fn reset_camera_on_stop(
    play_mode: Res<PlayModeState>,
    mut camera_state: ResMut<ScriptCameraState>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_playing() || play_mode.is_paused();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        // Reset camera state
        *camera_state = ScriptCameraState::default();
    }

    *last_playing = currently_playing;
}
