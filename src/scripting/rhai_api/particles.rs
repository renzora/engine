//! Particle effects API functions for Rhai scripts
//!
//! Provides control over bevy_hanabi particle effects from scripts.

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register particle effect functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Playback Control
    // ===================

    // particle_play(entity_id) - Start/resume playing the particle effect
    engine.register_fn("particle_play", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_play"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // particle_pause(entity_id) - Pause the particle effect
    engine.register_fn("particle_pause", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_pause"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // particle_stop(entity_id) - Stop and reset the particle effect
    engine.register_fn("particle_stop", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_stop"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // particle_reset(entity_id) - Reset the effect to initial state (keeps playing)
    engine.register_fn("particle_reset", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_reset"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // ===================
    // Burst Spawning
    // ===================

    // particle_burst(entity_id, count) - Emit a burst of particles
    engine.register_fn("particle_burst", |entity_id: i64, count: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_burst"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("count".into(), Dynamic::from(count));
        m
    });

    // ===================
    // Runtime Overrides
    // ===================

    // particle_set_rate(entity_id, multiplier) - Set spawn rate multiplier (1.0 = normal)
    engine.register_fn("particle_set_rate", |entity_id: i64, multiplier: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_rate"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("multiplier".into(), Dynamic::from(multiplier));
        m
    });

    // particle_set_scale(entity_id, multiplier) - Set particle size multiplier (1.0 = normal)
    engine.register_fn("particle_set_scale", |entity_id: i64, multiplier: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_scale"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("multiplier".into(), Dynamic::from(multiplier));
        m
    });

    // particle_set_time_scale(entity_id, scale) - Set effect time scale (1.0 = normal speed)
    engine.register_fn("particle_set_time_scale", |entity_id: i64, scale: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_time_scale"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("scale".into(), Dynamic::from(scale));
        m
    });

    // ===================
    // Color Tinting
    // ===================

    // particle_set_tint(entity_id, r, g, b, a) - Set color tint (RGBA, 1.0 = no tint)
    engine.register_fn("particle_set_tint", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_tint"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // particle_set_tint_rgb(entity_id, r, g, b) - Set color tint (RGB only, alpha = 1.0)
    engine.register_fn("particle_set_tint_rgb", |entity_id: i64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_tint"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(1.0));
        m
    });

    // particle_clear_tint(entity_id) - Remove color tint (set to white)
    engine.register_fn("particle_clear_tint", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_tint"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(1.0));
        m.insert("g".into(), Dynamic::from(1.0));
        m.insert("b".into(), Dynamic::from(1.0));
        m.insert("a".into(), Dynamic::from(1.0));
        m
    });

    // ===================
    // Custom Variables
    // ===================

    // particle_set_variable_float(entity_id, name, value) - Set a custom float variable
    engine.register_fn("particle_set_variable_float", |entity_id: i64, name: ImmutableString, value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_variable"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("var_type".into(), Dynamic::from("float"));
        m.insert("value".into(), Dynamic::from(value));
        m
    });

    // particle_set_variable_color(entity_id, name, r, g, b, a) - Set a custom color variable
    engine.register_fn("particle_set_variable_color", |entity_id: i64, name: ImmutableString, r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_variable"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("var_type".into(), Dynamic::from("color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // particle_set_variable_vec3(entity_id, name, x, y, z) - Set a custom vec3 variable
    engine.register_fn("particle_set_variable_vec3", |entity_id: i64, name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_set_variable"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("var_type".into(), Dynamic::from("vec3"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Convenience Functions
    // ===================

    // particle_emit_at(entity_id, x, y, z) - Move emitter and emit a burst
    // Useful for one-shot effects like explosions
    engine.register_fn("particle_emit_at", |entity_id: i64, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_emit_at"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // particle_emit_at_with_count(entity_id, x, y, z, count)
    engine.register_fn("particle_emit_at_with_count", |entity_id: i64, x: f64, y: f64, z: f64, count: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("particle_emit_at"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("count".into(), Dynamic::from(count));
        m
    });
}
