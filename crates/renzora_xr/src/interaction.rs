//! VR interaction systems — teleport, smooth locomotion, snap turn, grab
//!
//! All systems read from `VrControllerState` and `VrConfig` and modify
//! the `VrCameraRig` transform for locomotion or entity transforms for grab.

use bevy::prelude::*;

use crate::{
    VrConfig, VrHand, LocomotionMode,
    camera::VrCameraRig,
    components::{TeleportAreaData, VrGrabbableData, GrabType},
    input::VrControllerState,
    resources::vr_info,
};

// ============================================================================
// Teleport
// ============================================================================

/// Active teleport arc state
#[derive(Resource, Default)]
pub struct TeleportState {
    /// Whether the teleport arc is being shown
    pub active: bool,
    /// Valid hit point (if arc hits a TeleportArea surface)
    pub target: Option<Vec3>,
    /// Which hand is doing the teleport
    pub hand: VrHand,
}

/// System: teleport locomotion via parabolic arc.
///
/// When the locomotion hand thumbstick is pushed forward, cast a parabolic arc
/// from the controller aim pose. If it hits a surface with `TeleportAreaData`,
/// show a green indicator. On thumbstick release, move `VrCameraRig` to hit point.
pub fn teleport_system(
    config: Res<VrConfig>,
    controllers: Res<VrControllerState>,
    mut rig_query: Query<&mut Transform, With<VrCameraRig>>,
    _teleport_areas: Query<&TeleportAreaData>,
    mut teleport_state: Local<TeleportState>,
) {
    if !matches!(config.locomotion_mode, LocomotionMode::Teleport | LocomotionMode::Both) {
        return;
    }

    let hand = &controllers.hand(config.locomotion_hand);
    let thumbstick_y = hand.thumbstick_y;

    // Activate teleport arc when thumbstick pushed forward past deadzone
    if thumbstick_y > config.thumbstick_deadzone {
        teleport_state.active = true;
        teleport_state.hand = config.locomotion_hand;

        // Parabolic arc raycast from aim pose
        // In a full implementation, this would use Avian3D raycasts
        // to find the intersection with TeleportAreaData surfaces.
        //
        // For now, project forward and down from the aim pose:
        let aim_pos = hand.aim_position;
        let aim_rot = hand.aim_rotation;
        let forward = aim_rot * Vec3::NEG_Z;
        let _arc_endpoint = aim_pos + forward * 5.0 + Vec3::new(0.0, -2.0, 0.0);

        // TODO: actual parabolic arc raycast against TeleportAreaData colliders
        // teleport_state.target = Some(hit_point);
    } else if teleport_state.active && thumbstick_y < config.thumbstick_deadzone {
        // Thumbstick released — execute teleport if valid target
        if let Some(target) = teleport_state.target {
            for mut rig_transform in rig_query.iter_mut() {
                // Move rig to target, keeping current Y rotation
                rig_transform.translation.x = target.x;
                rig_transform.translation.z = target.z;
                // Optionally adjust Y to match surface height
                rig_transform.translation.y = target.y;
                vr_info(format!("Teleported to {:?}", target));
            }
        }
        teleport_state.active = false;
        teleport_state.target = None;
    }
}

// ============================================================================
// Smooth Locomotion
// ============================================================================

/// System: smooth locomotion via thumbstick.
///
/// Moves the `VrCameraRig` based on the locomotion hand's thumbstick input,
/// relative to the head's forward direction (projected onto XZ plane).
pub fn smooth_locomotion_system(
    config: Res<VrConfig>,
    controllers: Res<VrControllerState>,
    mut rig_query: Query<&mut Transform, With<VrCameraRig>>,
    head_query: Query<&GlobalTransform, With<crate::camera::VrHead>>,
    time: Res<Time>,
) {
    if !matches!(config.locomotion_mode, LocomotionMode::Smooth | LocomotionMode::Both) {
        return;
    }

    let hand = controllers.hand(config.locomotion_hand);
    let stick_x = hand.thumbstick_x;
    let stick_y = hand.thumbstick_y;

    // Apply deadzone
    let mag = (stick_x * stick_x + stick_y * stick_y).sqrt();
    if mag < config.thumbstick_deadzone {
        return;
    }

    // Get head forward direction projected onto XZ plane
    let head_forward = if let Ok(head_gt) = head_query.single() {
        let fwd = head_gt.forward().as_vec3();
        Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero()
    } else {
        Vec3::NEG_Z
    };

    // forward.cross(Y) = right vector (Y.cross(forward) would give left)
    let head_right = head_forward.cross(Vec3::Y).normalize_or_zero();

    // Movement direction relative to head
    // Negate stick_y: OpenXR Y+ = forward push, but head_forward already
    // points in -Z (Bevy forward), so we negate to match conventions.
    let movement = (head_forward * -stick_y + head_right * stick_x) * config.move_speed * time.delta_secs();

    for mut rig_transform in rig_query.iter_mut() {
        rig_transform.translation += movement;
    }
}

// ============================================================================
// Snap / Smooth Turn
// ============================================================================

/// System: snap or smooth turn via right thumbstick X-axis.
///
/// Rotates the `VrCameraRig` around Y-axis. If `snap_turn_angle > 0`,
/// applies discrete rotation with cooldown. If 0, applies smooth rotation.
pub fn snap_turn_system(
    config: Res<VrConfig>,
    controllers: Res<VrControllerState>,
    mut rig_query: Query<&mut Transform, With<VrCameraRig>>,
    time: Res<Time>,
    mut cooldown: Local<f32>,
) {
    // Turn hand is opposite of locomotion hand
    let turn_hand = match config.locomotion_hand {
        VrHand::Left => VrHand::Right,
        VrHand::Right => VrHand::Left,
    };
    let hand = controllers.hand(turn_hand);
    let stick_x = hand.thumbstick_x;

    // Tick cooldown
    if *cooldown > 0.0 {
        *cooldown -= time.delta_secs();
    }

    if stick_x.abs() < config.thumbstick_deadzone {
        // Reset cooldown when thumbstick returns to center (for snap turn)
        if config.snap_turn_angle > 0.0 {
            *cooldown = 0.0;
        }
        return;
    }

    // Snap turn threshold — require a firm push to trigger (prevents noise/bounce)
    const SNAP_TURN_THRESHOLD: f32 = 0.7;

    if config.snap_turn_angle > 0.0 {
        // Snap turn — only trigger when pushed past threshold
        if *cooldown <= 0.0 && stick_x.abs() > SNAP_TURN_THRESHOLD {
            let angle = if stick_x > 0.0 {
                -config.snap_turn_angle.to_radians()
            } else {
                config.snap_turn_angle.to_radians()
            };

            for mut rig_transform in rig_query.iter_mut() {
                rig_transform.rotate_y(angle);
            }

            *cooldown = config.snap_turn_cooldown;
        }
    } else {
        // Smooth turn — apply proportional rotation (deadzone already filtered above)
        let turn_speed = 120.0_f32.to_radians(); // degrees per second
        let angle = -stick_x * turn_speed * time.delta_secs();

        for mut rig_transform in rig_query.iter_mut() {
            rig_transform.rotate_y(angle);
        }
    }
}

// ============================================================================
// Trigger Vertical Movement (Q/E equivalent)
// ============================================================================

/// System: vertical camera movement via trigger buttons.
///
/// Right trigger → move up (like E key), left trigger → move down (like Q key).
/// Uses trigger analog value for proportional speed.
pub fn trigger_vertical_system(
    config: Res<VrConfig>,
    controllers: Res<VrControllerState>,
    mut rig_query: Query<&mut Transform, With<VrCameraRig>>,
    time: Res<Time>,
) {
    let left_trigger = controllers.left.trigger;
    let right_trigger = controllers.right.trigger;

    // Only activate past a small threshold to avoid drift
    const TRIGGER_THRESHOLD: f32 = 0.15;

    let mut vertical = 0.0;
    if right_trigger > TRIGGER_THRESHOLD {
        vertical += right_trigger; // up
    }
    if left_trigger > TRIGGER_THRESHOLD {
        vertical -= left_trigger; // down
    }

    if vertical.abs() < 0.01 {
        return;
    }

    let movement = Vec3::Y * vertical * config.move_speed * time.delta_secs();

    for mut rig_transform in rig_query.iter_mut() {
        rig_transform.translation += movement;
    }
}

// ============================================================================
// Grab Interaction
// ============================================================================

/// Currently grabbed object state
#[derive(Default)]
pub struct GrabState {
    /// Entity being grabbed by left hand
    left_grabbed: Option<Entity>,
    /// Entity being grabbed by right hand
    right_grabbed: Option<Entity>,
    /// Offset from grip to object center when grabbed
    left_offset: Transform,
    right_offset: Transform,
    /// Previous frame grip position for velocity calculation
    left_prev_pos: Vec3,
    right_prev_pos: Vec3,
}

/// System: grab and throw physics objects.
///
/// When grip button is pressed near a `VrGrabbableData` entity, attach it
/// to the hand. When released, apply throw velocity via `ExternalImpulse`.
pub fn grab_system(
    controllers: Res<VrControllerState>,
    grabbables: Query<(Entity, &GlobalTransform, &VrGrabbableData)>,
    mut transforms: Query<&mut Transform>,
    mut grab_state: Local<GrabState>,
    _time: Res<Time>,
) {
    // Deref Local once so the borrow checker can split fields
    let gs = &mut *grab_state;

    // Process left hand
    process_grab_hand(
        VrHand::Left,
        &controllers.left,
        &grabbables,
        &mut transforms,
        &mut gs.left_grabbed,
        &mut gs.left_offset,
        &mut gs.left_prev_pos,
    );

    // Process right hand
    process_grab_hand(
        VrHand::Right,
        &controllers.right,
        &grabbables,
        &mut transforms,
        &mut gs.right_grabbed,
        &mut gs.right_offset,
        &mut gs.right_prev_pos,
    );
}

fn process_grab_hand(
    hand: VrHand,
    controller: &crate::input::ControllerHandState,
    grabbables: &Query<(Entity, &GlobalTransform, &VrGrabbableData)>,
    transforms: &mut Query<&mut Transform>,
    grabbed: &mut Option<Entity>,
    offset: &mut Transform,
    prev_pos: &mut Vec3,
) {
    let grip_pressed = controller.grip_pressed;
    let grip_pos = controller.grip_position;
    let grip_rot = controller.grip_rotation;

    if grip_pressed {
        if grabbed.is_none() {
            // Try to grab nearest grabbable within reach
            let grab_radius = 0.15; // 15cm grab sphere
            let mut nearest: Option<(Entity, f32)> = None;

            for (entity, global_tf, _data) in grabbables.iter() {
                let dist = global_tf.translation().distance(grip_pos);
                if dist < grab_radius {
                    if nearest.is_none() || dist < nearest.unwrap().1 {
                        nearest = Some((entity, dist));
                    }
                }
            }

            if let Some((entity, _)) = nearest {
                *grabbed = Some(entity);
                // Calculate offset from grip to object
                if let Ok(obj_tf) = transforms.get(entity) {
                    let inv_grip = Transform::from_translation(grip_pos)
                        .with_rotation(grip_rot)
                        .compute_affine()
                        .inverse();
                    let obj_in_grip = Transform::from_matrix(
                        Mat4::from(inv_grip) * obj_tf.to_matrix(),
                    );
                    *offset = obj_in_grip;
                }
                vr_info(format!("Grabbed entity {:?} with {:?} hand", entity, hand));
            }
        }

        // Update grabbed object position
        if let Some(entity) = *grabbed {
            if let Ok(mut obj_tf) = transforms.get_mut(entity) {
                let grip_tf = Transform::from_translation(grip_pos)
                    .with_rotation(grip_rot);

                match grabbables.get(entity).map(|(_, _, d)| d.grab_type) {
                    Ok(GrabType::Snap) => {
                        // Snap to hand — no offset
                        obj_tf.translation = grip_tf.translation;
                        obj_tf.rotation = grip_tf.rotation;
                    }
                    _ => {
                        // Offset grab — maintain relative position
                        let target = Transform::from_matrix(
                            grip_tf.to_matrix() * offset.to_matrix(),
                        );
                        obj_tf.translation = target.translation;
                        obj_tf.rotation = target.rotation;
                    }
                }
            }
        }
    } else {
        // Grip released — drop/throw
        if let Some(entity) = grabbed.take() {
            let velocity = grip_pos - *prev_pos;
            let _ = (entity, velocity);
            vr_info(format!("Released entity with {:?} hand", hand));
        }
    }

    *prev_pos = grip_pos;
}
