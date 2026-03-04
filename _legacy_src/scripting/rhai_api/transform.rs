//! Transform API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};
use super::super::rhai_commands::RhaiCommand;

/// Register transform functions
pub fn register(engine: &mut Engine) {
    // set_position(x, y, z) - Set absolute position
    engine.register_fn("set_position", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetPosition { x: x as f32, y: y as f32, z: z as f32 });
    });

    // set_rotation(x, y, z) - Set rotation in degrees (euler angles)
    engine.register_fn("set_rotation", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetRotation { x: x as f32, y: y as f32, z: z as f32 });
    });

    // set_scale(x, y, z) - Set scale
    engine.register_fn("set_scale", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetScale { x: x as f32, y: y as f32, z: z as f32 });
    });

    // set_scale_uniform(s) - Set uniform scale
    engine.register_fn("set_scale_uniform", |s: f64| {
        super::push_command(RhaiCommand::SetScale { x: s as f32, y: s as f32, z: s as f32 });
    });

    // translate(x, y, z) - Move by delta in world space
    engine.register_fn("translate", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::Translate { x: x as f32, y: y as f32, z: z as f32 });
    });

    // rotate(x, y, z) - Rotate by delta in degrees
    engine.register_fn("rotate", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::Rotate { x: x as f32, y: y as f32, z: z as f32 });
    });

    // look_at(x, y, z) - Face a target position
    engine.register_fn("look_at", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::LookAt { x: x as f32, y: y as f32, z: z as f32 });
    });

    // ===================
    // Parent transform functions
    // ===================

    // parent_set_position(x, y, z)
    engine.register_fn("parent_set_position", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ParentSetPosition { x: x as f32, y: y as f32, z: z as f32 });
    });

    // parent_set_rotation(x, y, z)
    engine.register_fn("parent_set_rotation", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ParentSetRotation { x: x as f32, y: y as f32, z: z as f32 });
    });

    // parent_translate(x, y, z)
    engine.register_fn("parent_translate", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ParentTranslate { x: x as f32, y: y as f32, z: z as f32 });
    });

    // ===================
    // Child transform functions
    // ===================

    // set_child_position(name, x, y, z)
    engine.register_fn("set_child_position", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ChildSetPosition { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 });
    });

    // set_child_rotation(name, x, y, z)
    engine.register_fn("set_child_rotation", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ChildSetRotation { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 });
    });

    // child_translate(name, x, y, z)
    engine.register_fn("child_translate", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::ChildTranslate { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 });
    });

    // ===================
    // Vector helpers
    // ===================

    // vec3(x, y, z) - Create a vector map
    engine.register_fn("vec3", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // vec2(x, y) - Create a 2D vector map
    engine.register_fn("vec2", |x: f64, y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m
    });
}
