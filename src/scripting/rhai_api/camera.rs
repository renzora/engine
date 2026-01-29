//! Camera API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};

/// Register camera functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Camera Position/Target
    // ===================

    // set_camera_position(x, y, z) - Set camera world position
    engine.register_fn("set_camera_position", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_camera_position"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // set_camera_target(x, y, z) - Set camera look-at target
    engine.register_fn("set_camera_target", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_camera_target"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // camera_look_at(x, y, z) - Make camera look at position
    engine.register_fn("camera_look_at", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_look_at"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Camera Zoom/FOV
    // ===================

    // set_camera_zoom(zoom) - Set orthographic zoom or perspective distance
    engine.register_fn("set_camera_zoom", |zoom: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_camera_zoom"));
        m.insert("zoom".into(), Dynamic::from(zoom));
        m
    });

    // set_camera_fov(fov_degrees) - Set field of view for perspective camera
    engine.register_fn("set_camera_fov", |fov: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_camera_fov"));
        m.insert("fov".into(), Dynamic::from(fov));
        m
    });

    // ===================
    // Camera Effects
    // ===================

    // screen_shake(intensity, duration) - Shake the camera
    engine.register_fn("screen_shake", |intensity: f64, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("screen_shake"));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m.insert("duration".into(), Dynamic::from(duration));
        m
    });

    // screen_shake_once(intensity) - Quick one-frame shake
    engine.register_fn("screen_shake_once", |intensity: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("screen_shake"));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m.insert("duration".into(), Dynamic::from(0.1));
        m
    });

    // camera_flash(r, g, b, a, duration) - Flash screen with color
    engine.register_fn("camera_flash", |r: f64, g: f64, b: f64, a: f64, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_flash"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m.insert("duration".into(), Dynamic::from(duration));
        m
    });

    // ===================
    // Follow Camera
    // ===================

    // camera_follow(entity_id) - Make camera follow entity with default offset
    engine.register_fn("camera_follow", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // camera_follow_with_offset(entity_id, offset_x, offset_y, offset_z) - Make camera follow entity with offset
    engine.register_fn("camera_follow_with_offset", |entity_id: i64, offset_x: f64, offset_y: f64, offset_z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("offset_x".into(), Dynamic::from(offset_x));
        m.insert("offset_y".into(), Dynamic::from(offset_y));
        m.insert("offset_z".into(), Dynamic::from(offset_z));
        m
    });

    // camera_follow_with_params(entity_id, offset_x, offset_y, offset_z, smoothing) - Full control
    engine.register_fn("camera_follow_with_params", |entity_id: i64, offset_x: f64, offset_y: f64, offset_z: f64, smoothing: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("offset_x".into(), Dynamic::from(offset_x));
        m.insert("offset_y".into(), Dynamic::from(offset_y));
        m.insert("offset_z".into(), Dynamic::from(offset_z));
        m.insert("smoothing".into(), Dynamic::from(smoothing));
        m
    });

    // camera_follow_self() - Make camera follow self with default offset
    engine.register_fn("camera_follow_self", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow_self"));
        m
    });

    // camera_follow_self_with_offset(offset_x, offset_y, offset_z) - Make camera follow self with offset
    engine.register_fn("camera_follow_self_with_offset", |offset_x: f64, offset_y: f64, offset_z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow_self"));
        m.insert("offset_x".into(), Dynamic::from(offset_x));
        m.insert("offset_y".into(), Dynamic::from(offset_y));
        m.insert("offset_z".into(), Dynamic::from(offset_z));
        m
    });

    // camera_follow_self_with_params(offset_x, offset_y, offset_z, smoothing) - Full control
    engine.register_fn("camera_follow_self_with_params", |offset_x: f64, offset_y: f64, offset_z: f64, smoothing: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_follow_self"));
        m.insert("offset_x".into(), Dynamic::from(offset_x));
        m.insert("offset_y".into(), Dynamic::from(offset_y));
        m.insert("offset_z".into(), Dynamic::from(offset_z));
        m.insert("smoothing".into(), Dynamic::from(smoothing));
        m
    });

    // camera_stop_follow() - Stop following
    engine.register_fn("camera_stop_follow", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("camera_stop_follow"));
        m
    });

    // ===================
    // Screen Space
    // ===================

    // world_to_screen(world_x, world_y, world_z) - Convert world position to screen coordinates
    // Note: This requires accessing camera state, returns through scope variable
    engine.register_fn("world_to_screen", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_query".into(), Dynamic::from("world_to_screen"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // screen_to_ray(screen_x, screen_y) - Get ray from screen position
    engine.register_fn("screen_to_ray", |x: f64, y: f64| -> Map {
        let mut m = Map::new();
        m.insert("_query".into(), Dynamic::from("screen_to_ray"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m
    });
}
