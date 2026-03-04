//! Particle effects API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register particle effect functions
pub fn register(engine: &mut Engine) {
    engine.register_fn("particle_play", |entity_id: i64| {
        super::push_command(RhaiCommand::ParticlePlay { entity_id: entity_id as u64 });
    });

    engine.register_fn("particle_pause", |entity_id: i64| {
        super::push_command(RhaiCommand::ParticlePause { entity_id: entity_id as u64 });
    });

    engine.register_fn("particle_stop", |entity_id: i64| {
        super::push_command(RhaiCommand::ParticleStop { entity_id: entity_id as u64 });
    });

    engine.register_fn("particle_reset", |entity_id: i64| {
        super::push_command(RhaiCommand::ParticleReset { entity_id: entity_id as u64 });
    });

    engine.register_fn("particle_burst", |entity_id: i64, count: i64| {
        super::push_command(RhaiCommand::ParticleBurst { entity_id: entity_id as u64, count: count as u32 });
    });

    engine.register_fn("particle_set_rate", |entity_id: i64, multiplier: f64| {
        super::push_command(RhaiCommand::ParticleSetRate { entity_id: entity_id as u64, multiplier: multiplier as f32 });
    });

    engine.register_fn("particle_set_scale", |entity_id: i64, multiplier: f64| {
        super::push_command(RhaiCommand::ParticleSetScale { entity_id: entity_id as u64, multiplier: multiplier as f32 });
    });

    engine.register_fn("particle_set_time_scale", |entity_id: i64, scale: f64| {
        super::push_command(RhaiCommand::ParticleSetTimeScale { entity_id: entity_id as u64, scale: scale as f32 });
    });

    engine.register_fn("particle_set_tint", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::ParticleSetTint { entity_id: entity_id as u64, r: r as f32, g: g as f32, b: b as f32, a: a as f32 });
    });

    engine.register_fn("particle_set_tint_rgb", |entity_id: i64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::ParticleSetTint { entity_id: entity_id as u64, r: r as f32, g: g as f32, b: b as f32, a: 1.0 });
    });

    engine.register_fn("particle_clear_tint", |entity_id: i64| {
        super::push_command(RhaiCommand::ParticleSetTint { entity_id: entity_id as u64, r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
    });

    engine.register_fn("particle_set_variable_float", |entity_id: i64, name: ImmutableString, value: f64| {
        super::push_command(RhaiCommand::ParticleSetVariableFloat { entity_id: entity_id as u64, name: name.to_string(), value: value as f32 });
    });

    engine.register_fn("particle_set_variable_color", |entity_id: i64, name: ImmutableString, r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::ParticleSetVariableColor { entity_id: entity_id as u64, name: name.to_string(), r: r as f32, g: g as f32, b: b as f32, a: a as f32 });
    });

    engine.register_fn("particle_set_variable_vec3", |entity_id: i64, name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ParticleSetVariableVec3 { entity_id: entity_id as u64, name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 });
    });

    engine.register_fn("particle_emit_at", |entity_id: i64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ParticleEmitAt { entity_id: entity_id as u64, x: x as f32, y: y as f32, z: z as f32, count: None });
    });

    engine.register_fn("particle_emit_at_with_count", |entity_id: i64, x: f64, y: f64, z: f64, count: i64| {
        super::push_command(RhaiCommand::ParticleEmitAt { entity_id: entity_id as u64, x: x as f32, y: y as f32, z: z as f32, count: Some(count as u32) });
    });
}
