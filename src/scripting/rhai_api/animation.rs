//! Animation API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register animation functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Skeletal Animation
    // ===================

    engine.register_fn("play_animation", |name: ImmutableString| {
        super::push_command(RhaiCommand::PlayAnimation { entity_id: None, name: name.to_string(), looping: true, speed: 1.0 });
    });

    engine.register_fn("play_animation_once", |name: ImmutableString| {
        super::push_command(RhaiCommand::PlayAnimation { entity_id: None, name: name.to_string(), looping: false, speed: 1.0 });
    });

    engine.register_fn("play_animation_speed", |name: ImmutableString, speed: f64| {
        super::push_command(RhaiCommand::PlayAnimation { entity_id: None, name: name.to_string(), looping: true, speed: speed as f32 });
    });

    engine.register_fn("play_animation_on", |entity_id: i64, name: ImmutableString| {
        super::push_command(RhaiCommand::PlayAnimation { entity_id: Some(entity_id as u64), name: name.to_string(), looping: true, speed: 1.0 });
    });

    engine.register_fn("stop_animation", || {
        super::push_command(RhaiCommand::StopAnimation { entity_id: None });
    });

    engine.register_fn("stop_animation_on", |entity_id: i64| {
        super::push_command(RhaiCommand::StopAnimation { entity_id: Some(entity_id as u64) });
    });

    engine.register_fn("pause_animation", || {
        super::push_command(RhaiCommand::PauseAnimation { entity_id: None });
    });

    engine.register_fn("resume_animation", || {
        super::push_command(RhaiCommand::ResumeAnimation { entity_id: None });
    });

    engine.register_fn("set_animation_speed", |speed: f64| {
        super::push_command(RhaiCommand::SetAnimationSpeed { entity_id: None, speed: speed as f32 });
    });

    // ===================
    // Sprite Animation
    // ===================

    engine.register_fn("play_sprite_animation", |name: ImmutableString| {
        super::push_command(RhaiCommand::PlaySpriteAnimation { entity_id: None, name: name.to_string(), looping: true });
    });

    engine.register_fn("play_sprite_animation_once", |name: ImmutableString| {
        super::push_command(RhaiCommand::PlaySpriteAnimation { entity_id: None, name: name.to_string(), looping: false });
    });

    engine.register_fn("set_sprite_frame", |frame: i64| {
        super::push_command(RhaiCommand::SetSpriteFrame { entity_id: None, frame });
    });

    // ===================
    // Tweening
    // ===================

    engine.register_fn("tween_to", |property: ImmutableString, target: f64, duration: f64, easing: ImmutableString| {
        super::push_command(RhaiCommand::Tween { entity_id: None, property: property.to_string(), target: target as f32, duration: duration as f32, easing: easing.to_string() });
    });

    engine.register_fn("tween_position", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| {
        super::push_command(RhaiCommand::TweenPosition { entity_id: None, target: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32), duration: duration as f32, easing: easing.to_string() });
    });

    engine.register_fn("tween_rotation", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| {
        super::push_command(RhaiCommand::TweenRotation { entity_id: None, target: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32), duration: duration as f32, easing: easing.to_string() });
    });

    engine.register_fn("tween_scale", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| {
        super::push_command(RhaiCommand::TweenScale { entity_id: None, target: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32), duration: duration as f32, easing: easing.to_string() });
    });
}
