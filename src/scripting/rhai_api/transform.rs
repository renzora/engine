//! Transform API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};

/// Register transform functions
pub fn register(engine: &mut Engine) {
    // set_position(x, y, z) - Set absolute position
    engine.register_fn("set_position", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_position".into(), Dynamic::from(true));
        m.insert("_new_position_x".into(), Dynamic::from(x));
        m.insert("_new_position_y".into(), Dynamic::from(y));
        m.insert("_new_position_z".into(), Dynamic::from(z));
        m
    });

    // set_rotation(x, y, z) - Set rotation in degrees (euler angles)
    engine.register_fn("set_rotation", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_rotation".into(), Dynamic::from(true));
        m.insert("_new_rotation_x".into(), Dynamic::from(x));
        m.insert("_new_rotation_y".into(), Dynamic::from(y));
        m.insert("_new_rotation_z".into(), Dynamic::from(z));
        m
    });

    // set_scale(x, y, z) - Set scale
    engine.register_fn("set_scale", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_scale".into(), Dynamic::from(true));
        m.insert("_new_scale_x".into(), Dynamic::from(x));
        m.insert("_new_scale_y".into(), Dynamic::from(y));
        m.insert("_new_scale_z".into(), Dynamic::from(z));
        m
    });

    // set_scale_uniform(s) - Set uniform scale
    engine.register_fn("set_scale_uniform", |s: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_scale".into(), Dynamic::from(true));
        m.insert("_new_scale_x".into(), Dynamic::from(s));
        m.insert("_new_scale_y".into(), Dynamic::from(s));
        m.insert("_new_scale_z".into(), Dynamic::from(s));
        m
    });

    // translate(x, y, z) - Move by delta in world space
    engine.register_fn("translate", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_translate".into(), Dynamic::from(true));
        m.insert("_translate_x".into(), Dynamic::from(x));
        m.insert("_translate_y".into(), Dynamic::from(y));
        m.insert("_translate_z".into(), Dynamic::from(z));
        m
    });

    // rotate(x, y, z) - Rotate by delta in degrees
    engine.register_fn("rotate", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_rotate".into(), Dynamic::from(true));
        m.insert("_rotate_x".into(), Dynamic::from(x));
        m.insert("_rotate_y".into(), Dynamic::from(y));
        m.insert("_rotate_z".into(), Dynamic::from(z));
        m
    });

    // look_at(x, y, z) - Face a target position
    engine.register_fn("look_at", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_look_at".into(), Dynamic::from(true));
        m.insert("_look_at_x".into(), Dynamic::from(x));
        m.insert("_look_at_y".into(), Dynamic::from(y));
        m.insert("_look_at_z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Parent transform functions
    // ===================

    // parent_set_position(x, y, z)
    engine.register_fn("parent_set_position", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_parent_set_position".into(), Dynamic::from(true));
        m.insert("_parent_new_position_x".into(), Dynamic::from(x));
        m.insert("_parent_new_position_y".into(), Dynamic::from(y));
        m.insert("_parent_new_position_z".into(), Dynamic::from(z));
        m
    });

    // parent_set_rotation(x, y, z)
    engine.register_fn("parent_set_rotation", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_parent_set_rotation".into(), Dynamic::from(true));
        m.insert("_parent_new_rotation_x".into(), Dynamic::from(x));
        m.insert("_parent_new_rotation_y".into(), Dynamic::from(y));
        m.insert("_parent_new_rotation_z".into(), Dynamic::from(z));
        m
    });

    // parent_translate(x, y, z)
    engine.register_fn("parent_translate", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_parent_translate".into(), Dynamic::from(true));
        m.insert("_parent_translate_x".into(), Dynamic::from(x));
        m.insert("_parent_translate_y".into(), Dynamic::from(y));
        m.insert("_parent_translate_z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Child transform functions
    // ===================

    // set_child_position(name, x, y, z)
    engine.register_fn("set_child_position", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_child_cmd".into(), Dynamic::from("set_position"));
        m.insert("_child_name".into(), Dynamic::from(name));
        m.insert("_child_x".into(), Dynamic::from(x));
        m.insert("_child_y".into(), Dynamic::from(y));
        m.insert("_child_z".into(), Dynamic::from(z));
        m
    });

    // set_child_rotation(name, x, y, z)
    engine.register_fn("set_child_rotation", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_child_cmd".into(), Dynamic::from("set_rotation"));
        m.insert("_child_name".into(), Dynamic::from(name));
        m.insert("_child_x".into(), Dynamic::from(x));
        m.insert("_child_y".into(), Dynamic::from(y));
        m.insert("_child_z".into(), Dynamic::from(z));
        m
    });

    // child_translate(name, x, y, z)
    engine.register_fn("child_translate", |name: rhai::ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_child_cmd".into(), Dynamic::from("translate"));
        m.insert("_child_name".into(), Dynamic::from(name));
        m.insert("_child_x".into(), Dynamic::from(x));
        m.insert("_child_y".into(), Dynamic::from(y));
        m.insert("_child_z".into(), Dynamic::from(z));
        m
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
