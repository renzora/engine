//! Thumbstick locomotion: smooth movement on the left stick, snap (or smooth)
//! turning on the right stick. Both move the [`XrTrackingRoot`] — the entity
//! the whole tracked rig (eyes, controllers) hangs off — so physical
//! room-scale movement stays layered on top untouched.

use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;
use bevy_mod_xr::session::XrTrackingRoot;

use crate::{VrConfig, VrInput};

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (smooth_locomotion, snap_turn).chain());
}

/// Left thumbstick moves the tracking root relative to head forward direction.
fn smooth_locomotion(
    input: Res<VrInput>,
    config: Res<VrConfig>,
    time: Res<Time>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    xr_cameras: Query<&Transform, (With<XrCamera>, Without<XrTrackingRoot>)>,
) {
    let stick = input.left_thumbstick;
    if stick.length() < config.thumbstick_deadzone {
        return;
    }

    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };

    // Head forward projected onto the XZ plane — "walk where I look". The eye
    // camera transforms are root-relative, so rotate into root space first.
    let head_forward = if let Some(cam_tf) = xr_cameras.iter().next() {
        let fwd = (root_tf.rotation * cam_tf.forward().as_vec3()).with_y(0.0);
        fwd.normalize_or_zero()
    } else {
        root_tf.forward().as_vec3()
    };

    let head_right = Vec3::new(-head_forward.z, 0.0, head_forward.x);
    let movement =
        (head_forward * stick.y + head_right * stick.x) * config.move_speed * time.delta_secs();

    root_tf.translation += movement;
}

/// Right thumbstick X-axis rotates the tracking root (snap or smooth turn).
fn snap_turn(
    input: Res<VrInput>,
    config: Res<VrConfig>,
    time: Res<Time>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();

    let stick_x = input.right_thumbstick.x;
    if config.smooth_turn {
        if stick_x.abs() < config.thumbstick_deadzone {
            return;
        }
        let Ok(mut root_tf) = tracking_root.single_mut() else {
            return;
        };
        let turn_speed = 90.0_f32.to_radians();
        root_tf.rotate_y(-stick_x * turn_speed * time.delta_secs());
        return;
    }

    // Snap turn: fire once past the threshold, then hold off until the stick
    // returns to center (or the cooldown lapses for continuous flicking).
    if stick_x.abs() < 0.3 {
        *cooldown = 0.0;
    }
    if stick_x.abs() < 0.7 || *cooldown > 0.0 {
        return;
    }
    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };
    let angle = if stick_x > 0.0 {
        -config.snap_turn_angle.to_radians()
    } else {
        config.snap_turn_angle.to_radians()
    };
    root_tf.rotate_y(angle);
    *cooldown = config.snap_turn_cooldown;
}
