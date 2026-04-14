use avian3d::prelude::*;
use bevy::prelude::*;

use crate::character_controller::*;
use crate::properties::PhysicsPropertiesState;

/// Ground detection using Avian's `SpatialQuery`.
pub fn character_ground_check(
    spatial_query: SpatialQuery,
    mut controllers: Query<(
        Entity,
        &CharacterControllerData,
        &mut CharacterControllerState,
        &GlobalTransform,
        &mut Transform,
        Option<&Collider>,
    )>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (entity, cc_data, mut state, global_transform, mut transform, collider) in &mut controllers {
        state.was_grounded = state.is_grounded;

        // Raycast origin: bottom of the character
        let position = global_transform.translation();
        // Use collider half-height if available, otherwise estimate
        let half_height = collider
            .map(|c| {
                let aabb = c.aabb(position, Rotation::default());
                (aabb.max.y - aabb.min.y) * 0.5
            })
            .unwrap_or(0.9);

        let ray_origin = position;
        let ray_direction = Dir3::NEG_Y;
        // Extend the ray by next-frame downward displacement so the character
        // can't teleport past the surface between ticks when gravity has
        // accelerated it. Also adds a generous floor so slow-moving objects
        // still resnap on first ground contact.
        let lookahead = state.velocity.y.min(0.0).abs() * dt * 2.0;
        let max_distance = half_height + cc_data.ground_distance + lookahead;

        // Cast ray downward, excluding self
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        if let Some(hit) = spatial_query.cast_ray(
            ray_origin,
            ray_direction,
            max_distance,
            true,
            &filter,
        ) {
            let normal = hit.normal;
            let slope_angle = normal.angle_between(Vec3::Y).to_degrees();

            if slope_angle <= cc_data.max_slope_angle {
                state.is_grounded = true;
                state.ground_normal = normal;
                state.airborne_timer = 0.0;

                // Snap the character's feet to the ground. Without this, after
                // a frame where gravity has pushed the body below the surface,
                // the body would stay below since we only kill downward
                // velocity (it never climbs back up).
                let feet_y = position.y - half_height;
                let ground_y = position.y - hit.distance;
                let penetration = ground_y - feet_y;
                if penetration.abs() > 0.0001 {
                    transform.translation.y += penetration;
                }
            } else {
                // On a slope too steep to stand on
                state.is_grounded = false;
                state.ground_normal = Vec3::Y;
                state.airborne_timer += dt;
            }
        } else {
            state.is_grounded = false;
            state.ground_normal = Vec3::Y;
            state.airborne_timer += dt;
        }
    }
}

/// Movement and jump logic.
pub fn character_movement(
    mut controllers: Query<(
        &CharacterControllerData,
        &mut CharacterControllerState,
        &CharacterControllerInput,
        &GlobalTransform,
    )>,
    physics_props: Res<PhysicsPropertiesState>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }
    let world_gravity = physics_props.gravity;

    for (cc_data, mut state, input, global_transform) in &mut controllers {
        // --- Horizontal movement ---
        let move_dir = if input.movement.length_squared() > 0.001 {
            // Convert 2D input to 3D world direction relative to entity facing
            let forward = global_transform.forward().as_vec3();
            let right = global_transform.right().as_vec3();
            // Project to XZ plane
            let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right_xz = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
            (forward_xz * input.movement.y + right_xz * input.movement.x).normalize_or_zero()
        } else {
            Vec3::ZERO
        };

        let speed = cc_data.move_speed * if input.sprint { cc_data.sprint_multiplier } else { 1.0 };
        let control = if state.is_grounded || state.airborne_timer < cc_data.coyote_time {
            1.0
        } else {
            cc_data.air_control
        };

        // Set horizontal velocity directly (no acceleration model — snappy feel)
        let target_horizontal = move_dir * speed * control;
        state.velocity.x = target_horizontal.x;
        state.velocity.z = target_horizontal.z;

        // --- Gravity ---
        let gravity = world_gravity * cc_data.gravity_scale;
        if !state.is_grounded {
            state.velocity += gravity * dt;
        } else if state.velocity.y < 0.0 {
            // Snap to ground
            state.velocity.y = 0.0;
        }

        // --- Jump buffering ---
        if input.jump {
            state.jump_buffer_timer = 0.0;
        } else {
            state.jump_buffer_timer += dt;
        }

        // --- Jump execution ---
        let can_jump = state.is_grounded || state.airborne_timer < cc_data.coyote_time;
        let has_buffered_jump = state.jump_buffer_timer < cc_data.jump_buffer_time;

        if can_jump && has_buffered_jump {
            state.velocity.y = cc_data.jump_force;
            state.is_grounded = false;
            state.airborne_timer = cc_data.coyote_time; // Consume coyote time
            state.jump_buffer_timer = cc_data.jump_buffer_time; // Consume buffer
        }
    }
}

/// Apply the controller's velocity by moving the transform directly.
///
/// Kinematic bodies in Avian are not moved by LinearVelocity — they must be
/// repositioned explicitly. We also set LinearVelocity so Avian's collision
/// response knows the body is moving.
pub fn character_apply_velocity(
    mut controllers: Query<(
        &CharacterControllerState,
        &mut Transform,
        Option<&mut LinearVelocity>,
    )>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }
    for (state, mut transform, linear_vel) in &mut controllers {
        transform.translation += state.velocity * dt;

        // Also set LinearVelocity so Avian's CCD and collision response
        // knows this body is moving.
        if let Some(mut lv) = linear_vel {
            lv.0 = state.velocity;
        }
    }
}
