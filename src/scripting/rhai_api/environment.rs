//! Environment API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};

/// Register environment functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Sun/Directional Light
    // ===================

    // set_sun_angles(azimuth, elevation) - Set sun position in degrees
    engine.register_fn("set_sun_angles", |azimuth: f64, elevation: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_sun_angles".into(), Dynamic::from(true));
        m.insert("_sun_azimuth".into(), Dynamic::from(azimuth));
        m.insert("_sun_elevation".into(), Dynamic::from(elevation));
        m
    });

    // set_sun_direction(x, y, z) - Set sun direction directly
    engine.register_fn("set_sun_direction", |x: f64, y: f64, z: f64| -> Map {
        // Convert direction to azimuth/elevation
        let azimuth = (-x).atan2(-z).to_degrees();
        let horizontal = (x*x + z*z).sqrt();
        let elevation = (-y).atan2(horizontal).to_degrees();

        let mut m = Map::new();
        m.insert("_set_sun_angles".into(), Dynamic::from(true));
        m.insert("_sun_azimuth".into(), Dynamic::from(azimuth));
        m.insert("_sun_elevation".into(), Dynamic::from(elevation));
        m
    });

    // ===================
    // Ambient Light
    // ===================

    // set_ambient_brightness(value) - Set ambient light brightness
    engine.register_fn("set_ambient_brightness", |value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_ambient_brightness".into(), Dynamic::from(true));
        m.insert("_ambient_brightness".into(), Dynamic::from(value));
        m
    });

    // set_ambient_color(r, g, b) - Set ambient light color
    engine.register_fn("set_ambient_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_ambient_color".into(), Dynamic::from(true));
        m.insert("_ambient_color_r".into(), Dynamic::from(r));
        m.insert("_ambient_color_g".into(), Dynamic::from(g));
        m.insert("_ambient_color_b".into(), Dynamic::from(b));
        m
    });

    // ===================
    // Sky
    // ===================

    // set_sky_top_color(r, g, b) - Set procedural sky top color
    engine.register_fn("set_sky_top_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_sky_top_color".into(), Dynamic::from(true));
        m.insert("_sky_top_r".into(), Dynamic::from(r));
        m.insert("_sky_top_g".into(), Dynamic::from(g));
        m.insert("_sky_top_b".into(), Dynamic::from(b));
        m
    });

    // set_sky_horizon_color(r, g, b) - Set procedural sky horizon color
    engine.register_fn("set_sky_horizon_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_sky_horizon_color".into(), Dynamic::from(true));
        m.insert("_sky_horizon_r".into(), Dynamic::from(r));
        m.insert("_sky_horizon_g".into(), Dynamic::from(g));
        m.insert("_sky_horizon_b".into(), Dynamic::from(b));
        m
    });

    // ===================
    // Fog
    // ===================

    // set_fog(enabled, start, end) - Configure fog
    engine.register_fn("set_fog", |enabled: bool, start: f64, end: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_fog".into(), Dynamic::from(true));
        m.insert("_fog_enabled".into(), Dynamic::from(enabled));
        m.insert("_fog_start".into(), Dynamic::from(start));
        m.insert("_fog_end".into(), Dynamic::from(end));
        m
    });

    // enable_fog(start, end)
    engine.register_fn("enable_fog", |start: f64, end: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_fog".into(), Dynamic::from(true));
        m.insert("_fog_enabled".into(), Dynamic::from(true));
        m.insert("_fog_start".into(), Dynamic::from(start));
        m.insert("_fog_end".into(), Dynamic::from(end));
        m
    });

    // disable_fog()
    engine.register_fn("disable_fog", || -> Map {
        let mut m = Map::new();
        m.insert("_set_fog".into(), Dynamic::from(true));
        m.insert("_fog_enabled".into(), Dynamic::from(false));
        m.insert("_fog_start".into(), Dynamic::from(10.0));
        m.insert("_fog_end".into(), Dynamic::from(100.0));
        m
    });

    // set_fog_color(r, g, b) - Set fog color
    engine.register_fn("set_fog_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_fog_color".into(), Dynamic::from(true));
        m.insert("_fog_color_r".into(), Dynamic::from(r));
        m.insert("_fog_color_g".into(), Dynamic::from(g));
        m.insert("_fog_color_b".into(), Dynamic::from(b));
        m
    });

    // ===================
    // Exposure
    // ===================

    // set_ev100(value) - Set camera exposure (higher = darker, ~9.7 for outdoor)
    engine.register_fn("set_ev100", |value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_ev100".into(), Dynamic::from(true));
        m.insert("_ev100".into(), Dynamic::from(value));
        m
    });

    // set_exposure(value) - Alias for set_ev100
    engine.register_fn("set_exposure", |value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_set_ev100".into(), Dynamic::from(true));
        m.insert("_ev100".into(), Dynamic::from(value));
        m
    });
}
