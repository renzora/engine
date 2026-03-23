//! VR editor locomotion — thumbstick-based scene navigation.
//!
//! Moves and rotates `XrTrackingRoot` so the user can fly through the scene.
//! Only active in editor mode (skipped when `VrCameraRig` exists = play mode).
//!
//! Controls:
//! - **Left stick**: Forward/backward + strafe (head-relative)
//! - **Right stick X**: Smooth yaw rotation
//! - **Right stick Y**: Vertical movement (up/down)
//! - **Grip buttons** (squeeze): Right grip = ascend, left grip = descend
//! - **Trigger** (index finger): Reserved for panel grab (see interaction.rs)
//!
//! All movement uses velocity smoothing for comfortable VR locomotion.

use bevy::prelude::*;
use renzora_xr::reexports::{OxrViews, XrTrackingRoot};
use renzora_xr::{VrConfig, VrControllerState, VrCameraRig};

use crate::{VrEditorState, VrLocomotionSmoothing, VrPointerHit};

/// Apply deadzone: returns 0.0 if |value| < deadzone, otherwise rescaled to [0,1].
fn apply_deadzone(value: f32, deadzone: f32) -> f32 {
    if value.abs() < deadzone {
        0.0
    } else {
        (value.abs() - deadzone) / (1.0 - deadzone) * value.signum()
    }
}

/// Get the user's head-forward direction in **world space** (projected onto XZ).
fn head_forward_world_xz(views: &OxrViews, root_rotation: Quat) -> Option<Vec3> {
    if views.is_empty() {
        return None;
    }
    let o = &views[0].pose.orientation;
    let head_rot = Quat::from_xyzw(o.x, o.y, o.z, o.w);
    let world_rot = root_rotation * head_rot;
    let forward = world_rot * Vec3::NEG_Z;
    let flat = Vec3::new(forward.x, 0.0, forward.z);
    if flat.length_squared() < 1e-6 {
        None
    } else {
        Some(flat.normalize())
    }
}

/// Smoothing factor — higher = snappier, lower = smoother.
/// At 0.08 the velocity ramps up/down over ~4-5 frames at 72fps.
const SMOOTHING: f32 = 0.08;

/// System: left thumbstick → strafe/forward, right stick Y + grip → vertical.
///
/// Uses velocity smoothing for comfortable acceleration/deceleration.
pub fn editor_locomotion(
    config: Option<Res<VrConfig>>,
    controllers: Option<Res<VrControllerState>>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    views: Option<Res<OxrViews>>,
    play_mode_rigs: Query<Entity, With<VrCameraRig>>,
    time: Res<Time>,
    mut smoothing: ResMut<VrLocomotionSmoothing>,
    pointer_hit: Res<VrPointerHit>,
    vr_state: Res<VrEditorState>,
) {
    if !play_mode_rigs.is_empty() {
        return;
    }

    let Some(config) = config else { return };
    let Some(controllers) = controllers else { return };
    let Some(views) = views else { return };

    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };

    let dz = config.thumbstick_deadzone;
    let strafe = apply_deadzone(controllers.left.thumbstick_x, dz);
    let forward = apply_deadzone(controllers.left.thumbstick_y, dz);
    let right_stick_vertical = apply_deadzone(controllers.right.thumbstick_y, dz);

    // Suppress grip-based elevation when a hand is pointing at a panel or
    // actively grabbing one — prevents accidental flight while moving panels.
    let right_on_panel = pointer_hit.right.hit_entity.is_some()
        || vr_state.right.grabbed_panel.is_some();
    let left_on_panel = pointer_hit.left.hit_entity.is_some()
        || vr_state.left.grabbed_panel.is_some();

    const GRIP_THRESHOLD: f32 = 0.15;
    let mut vertical = right_stick_vertical;
    if controllers.right.grip > GRIP_THRESHOLD && !right_on_panel {
        vertical += controllers.right.grip;
    }
    if controllers.left.grip > GRIP_THRESHOLD && !left_on_panel {
        vertical -= controllers.left.grip;
    }

    let speed = config.move_speed;

    // Compute target velocity in world space
    let mut target_vel = Vec3::ZERO;
    if let Some(head_fwd) = head_forward_world_xz(&views, root_tf.rotation) {
        let head_right = Vec3::new(-head_fwd.z, 0.0, head_fwd.x);
        target_vel += head_fwd * forward * speed;
        target_vel += head_right * strafe * speed;
    }
    target_vel.y += vertical * speed;

    // Lerp velocity toward target for smooth acceleration/deceleration
    smoothing.velocity = smoothing.velocity.lerp(target_vel, SMOOTHING);

    // Stop completely when velocity is negligible
    if smoothing.velocity.length_squared() < 1e-6 {
        smoothing.velocity = Vec3::ZERO;
        return;
    }

    root_tf.translation += smoothing.velocity * time.delta_secs();
}

/// System: right thumbstick X → smooth yaw rotation with velocity lerping.
pub fn editor_camera_rotation(
    config: Option<Res<VrConfig>>,
    controllers: Option<Res<VrControllerState>>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    play_mode_rigs: Query<Entity, With<VrCameraRig>>,
    time: Res<Time>,
    mut smoothing: ResMut<VrLocomotionSmoothing>,
) {
    if !play_mode_rigs.is_empty() {
        return;
    }

    let Some(config) = config else { return };
    let Some(controllers) = controllers else { return };

    let dz = config.thumbstick_deadzone;
    let yaw_input = apply_deadzone(controllers.right.thumbstick_x, dz);

    let yaw_speed = 150.0_f32.to_radians();
    let target_yaw = -yaw_input * yaw_speed;

    // Lerp angular velocity for smooth start/stop
    smoothing.yaw_velocity = smoothing.yaw_velocity + (target_yaw - smoothing.yaw_velocity) * SMOOTHING;

    if smoothing.yaw_velocity.abs() < 1e-5 {
        smoothing.yaw_velocity = 0.0;
        return;
    }

    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };

    root_tf.rotate_y(smoothing.yaw_velocity * time.delta_secs());
}
