//! Physics API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register physics functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Forces & Impulses
    // ===================

    // apply_force(x, y, z) - Apply force to self
    engine.register_fn("apply_force", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ApplyForce { entity_id: None, force: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // apply_force_to(entity_id, x, y, z) - Apply force to entity
    engine.register_fn("apply_force_to", |entity_id: i64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ApplyForce { entity_id: Some(entity_id as u64), force: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // apply_impulse(x, y, z) - Apply instant impulse to self
    engine.register_fn("apply_impulse", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ApplyImpulse { entity_id: None, impulse: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // apply_impulse_to(entity_id, x, y, z) - Apply instant impulse to entity
    engine.register_fn("apply_impulse_to", |entity_id: i64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ApplyImpulse { entity_id: Some(entity_id as u64), impulse: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // apply_torque(x, y, z) - Apply rotational force to self
    engine.register_fn("apply_torque", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ApplyTorque { entity_id: None, torque: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // ===================
    // Velocity
    // ===================

    // set_velocity(x, y, z) - Set linear velocity of self
    engine.register_fn("set_velocity", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetVelocity { entity_id: None, velocity: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // set_velocity_of(entity_id, x, y, z) - Set velocity of entity
    engine.register_fn("set_velocity_of", |entity_id: i64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetVelocity { entity_id: Some(entity_id as u64), velocity: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // set_angular_velocity(x, y, z) - Set angular velocity of self
    engine.register_fn("set_angular_velocity", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetAngularVelocity { entity_id: None, velocity: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // ===================
    // Gravity
    // ===================

    // set_gravity_scale(scale) - Set gravity scale for self (1.0 = normal, 0.0 = no gravity)
    engine.register_fn("set_gravity_scale", |scale: f64| {
        super::push_command(RhaiCommand::SetGravityScale { entity_id: None, scale: scale as f32 });
    });

    // ===================
    // Raycasting
    // ===================

    // raycast(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z, max_dist, result_var)
    engine.register_fn("raycast", |
        ox: f64, oy: f64, oz: f64,
        dx: f64, dy: f64, dz: f64,
        max_dist: f64,
        result_var: ImmutableString
    | {
        super::push_command(RhaiCommand::Raycast {
            origin: bevy::prelude::Vec3::new(ox as f32, oy as f32, oz as f32),
            direction: bevy::prelude::Vec3::new(dx as f32, dy as f32, dz as f32),
            max_distance: max_dist as f32,
            result_var: result_var.to_string(),
        });
    });

    // raycast_down(origin_x, origin_y, origin_z, max_dist) - Shorthand for downward raycast
    engine.register_fn("raycast_down", |
        ox: f64, oy: f64, oz: f64,
        max_dist: f64,
        result_var: ImmutableString
    | {
        super::push_command(RhaiCommand::Raycast {
            origin: bevy::prelude::Vec3::new(ox as f32, oy as f32, oz as f32),
            direction: bevy::prelude::Vec3::new(0.0, -1.0, 0.0),
            max_distance: max_dist as f32,
            result_var: result_var.to_string(),
        });
    });
}
