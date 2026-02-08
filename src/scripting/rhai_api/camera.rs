//! Camera API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};
use super::super::rhai_commands::RhaiCommand;

/// Register camera functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Camera Position/Target
    // ===================

    // set_camera_position(x, y, z) - Set camera world position
    engine.register_fn("set_camera_position", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetCameraTarget { position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // set_camera_target(x, y, z) - Set camera look-at target
    engine.register_fn("set_camera_target", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetCameraTarget { position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // camera_look_at(x, y, z) - Make camera look at position
    engine.register_fn("camera_look_at", |x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SetCameraTarget { position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // ===================
    // Camera Zoom/FOV
    // ===================

    // set_camera_zoom(zoom) - Set orthographic zoom or perspective distance
    engine.register_fn("set_camera_zoom", |zoom: f64| {
        super::push_command(RhaiCommand::SetCameraZoom { zoom: zoom as f32 });
    });

    // set_camera_fov(fov_degrees) - Set field of view for perspective camera
    engine.register_fn("set_camera_fov", |_fov: f64| {
        // FOV is handled through the camera component
    });

    // ===================
    // Camera Effects
    // ===================

    // screen_shake(intensity, duration) - Shake the camera
    engine.register_fn("screen_shake", |intensity: f64, duration: f64| {
        super::push_command(RhaiCommand::ScreenShake { intensity: intensity as f32, duration: duration as f32 });
    });

    // screen_shake_once(intensity) - Quick one-frame shake
    engine.register_fn("screen_shake_once", |intensity: f64| {
        super::push_command(RhaiCommand::ScreenShake { intensity: intensity as f32, duration: 0.1 });
    });

    // camera_flash(r, g, b, a, duration) - Flash screen with color (placeholder)
    engine.register_fn("camera_flash", |_r: f64, _g: f64, _b: f64, _a: f64, _duration: f64| {});

    // ===================
    // Follow Camera
    // ===================

    // camera_follow(entity_id) - Make camera follow entity with default offset
    engine.register_fn("camera_follow", |entity_id: i64| {
        super::push_command(RhaiCommand::CameraFollow { entity_id: entity_id as u64, offset: bevy::prelude::Vec3::new(0.0, 5.0, -10.0), smoothing: 5.0 });
    });

    // camera_follow_with_offset(entity_id, offset_x, offset_y, offset_z)
    engine.register_fn("camera_follow_with_offset", |entity_id: i64, offset_x: f64, offset_y: f64, offset_z: f64| {
        super::push_command(RhaiCommand::CameraFollow { entity_id: entity_id as u64, offset: bevy::prelude::Vec3::new(offset_x as f32, offset_y as f32, offset_z as f32), smoothing: 5.0 });
    });

    // camera_follow_with_params(entity_id, offset_x, offset_y, offset_z, smoothing)
    engine.register_fn("camera_follow_with_params", |entity_id: i64, offset_x: f64, offset_y: f64, offset_z: f64, smoothing: f64| {
        super::push_command(RhaiCommand::CameraFollow { entity_id: entity_id as u64, offset: bevy::prelude::Vec3::new(offset_x as f32, offset_y as f32, offset_z as f32), smoothing: smoothing as f32 });
    });

    // camera_follow_self() - Make camera follow self with default offset
    engine.register_fn("camera_follow_self", || {
        // Self-follow uses entity_id 0 as sentinel, resolved at command processing time
        super::push_command(RhaiCommand::CameraFollow { entity_id: 0, offset: bevy::prelude::Vec3::new(0.0, 5.0, -10.0), smoothing: 5.0 });
    });

    // camera_follow_self_with_offset(offset_x, offset_y, offset_z)
    engine.register_fn("camera_follow_self_with_offset", |offset_x: f64, offset_y: f64, offset_z: f64| {
        super::push_command(RhaiCommand::CameraFollow { entity_id: 0, offset: bevy::prelude::Vec3::new(offset_x as f32, offset_y as f32, offset_z as f32), smoothing: 5.0 });
    });

    // camera_follow_self_with_params(offset_x, offset_y, offset_z, smoothing)
    engine.register_fn("camera_follow_self_with_params", |offset_x: f64, offset_y: f64, offset_z: f64, smoothing: f64| {
        super::push_command(RhaiCommand::CameraFollow { entity_id: 0, offset: bevy::prelude::Vec3::new(offset_x as f32, offset_y as f32, offset_z as f32), smoothing: smoothing as f32 });
    });

    // camera_stop_follow() - Stop following
    engine.register_fn("camera_stop_follow", || {
        super::push_command(RhaiCommand::StopCameraFollow);
    });

    // ===================
    // Screen Space
    // ===================

    // world_to_screen(world_x, world_y, world_z)
    engine.register_fn("world_to_screen", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_query".into(), Dynamic::from("world_to_screen"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // screen_to_ray(screen_x, screen_y)
    engine.register_fn("screen_to_ray", |x: f64, y: f64| -> Map {
        let mut m = Map::new();
        m.insert("_query".into(), Dynamic::from("screen_to_ray"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m
    });
}
