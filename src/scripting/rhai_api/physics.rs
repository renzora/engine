//! Physics API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register physics functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Forces & Impulses
    // ===================

    // apply_force(x, y, z) - Apply force to self
    engine.register_fn("apply_force", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("apply_force"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // apply_force_to(entity_id, x, y, z) - Apply force to entity
    engine.register_fn("apply_force_to", |entity_id: i64, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("apply_force"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // apply_impulse(x, y, z) - Apply instant impulse to self
    engine.register_fn("apply_impulse", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("apply_impulse"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // apply_impulse_to(entity_id, x, y, z) - Apply instant impulse to entity
    engine.register_fn("apply_impulse_to", |entity_id: i64, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("apply_impulse"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // apply_torque(x, y, z) - Apply rotational force to self
    engine.register_fn("apply_torque", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("apply_torque"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Velocity
    // ===================

    // set_velocity(x, y, z) - Set linear velocity of self
    engine.register_fn("set_velocity", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_velocity"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // set_velocity_of(entity_id, x, y, z) - Set velocity of entity
    engine.register_fn("set_velocity_of", |entity_id: i64, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_velocity"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // set_angular_velocity(x, y, z) - Set angular velocity of self
    engine.register_fn("set_angular_velocity", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_angular_velocity"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Gravity
    // ===================

    // set_gravity_scale(scale) - Set gravity scale for self (1.0 = normal, 0.0 = no gravity)
    engine.register_fn("set_gravity_scale", |scale: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_gravity_scale"));
        m.insert("scale".into(), Dynamic::from(scale));
        m
    });

    // ===================
    // Raycasting
    // ===================

    // raycast(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z, max_dist, result_var)
    // Result is stored in a variable that can be accessed
    engine.register_fn("raycast", |
        ox: f64, oy: f64, oz: f64,
        dx: f64, dy: f64, dz: f64,
        max_dist: f64,
        result_var: ImmutableString
    | -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("raycast"));
        m.insert("ox".into(), Dynamic::from(ox));
        m.insert("oy".into(), Dynamic::from(oy));
        m.insert("oz".into(), Dynamic::from(oz));
        m.insert("dx".into(), Dynamic::from(dx));
        m.insert("dy".into(), Dynamic::from(dy));
        m.insert("dz".into(), Dynamic::from(dz));
        m.insert("max_dist".into(), Dynamic::from(max_dist));
        m.insert("result_var".into(), Dynamic::from(result_var));
        m
    });

    // raycast_down(origin_x, origin_y, origin_z, max_dist) - Shorthand for downward raycast
    engine.register_fn("raycast_down", |
        ox: f64, oy: f64, oz: f64,
        max_dist: f64,
        result_var: ImmutableString
    | -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("raycast"));
        m.insert("ox".into(), Dynamic::from(ox));
        m.insert("oy".into(), Dynamic::from(oy));
        m.insert("oz".into(), Dynamic::from(oz));
        m.insert("dx".into(), Dynamic::from(0.0));
        m.insert("dy".into(), Dynamic::from(-1.0));
        m.insert("dz".into(), Dynamic::from(0.0));
        m.insert("max_dist".into(), Dynamic::from(max_dist));
        m.insert("result_var".into(), Dynamic::from(result_var));
        m
    });
}
