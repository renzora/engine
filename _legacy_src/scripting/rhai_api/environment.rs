//! Environment API functions for Rhai scripts

use rhai::Engine;
use super::super::rhai_commands::RhaiCommand;

/// Register environment functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Sun/Directional Light
    // ===================

    // set_sun_angles(azimuth, elevation) - Set sun position in degrees
    engine.register_fn("set_sun_angles", |azimuth: f64, elevation: f64| {
        super::push_command(RhaiCommand::SetSunAngles { azimuth: azimuth as f32, elevation: elevation as f32 });
    });

    // set_sun_direction(x, y, z) - Set sun direction directly
    engine.register_fn("set_sun_direction", |x: f64, y: f64, z: f64| {
        // Convert direction to azimuth/elevation
        let azimuth = (-x).atan2(-z).to_degrees();
        let horizontal = (x*x + z*z).sqrt();
        let elevation = (-y).atan2(horizontal).to_degrees();
        super::push_command(RhaiCommand::SetSunAngles { azimuth: azimuth as f32, elevation: elevation as f32 });
    });

    // ===================
    // Ambient Light
    // ===================

    // set_ambient_brightness(value) - Set ambient light brightness
    engine.register_fn("set_ambient_brightness", |value: f64| {
        super::push_command(RhaiCommand::SetAmbientBrightness { brightness: value as f32 });
    });

    // set_ambient_color(r, g, b) - Set ambient light color
    engine.register_fn("set_ambient_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetAmbientColor { r: r as f32, g: g as f32, b: b as f32 });
    });

    // ===================
    // Sky
    // ===================

    // set_sky_top_color(r, g, b) - Set procedural sky top color
    engine.register_fn("set_sky_top_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetSkyTopColor { r: r as f32, g: g as f32, b: b as f32 });
    });

    // set_sky_horizon_color(r, g, b) - Set procedural sky horizon color
    engine.register_fn("set_sky_horizon_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetSkyHorizonColor { r: r as f32, g: g as f32, b: b as f32 });
    });

    // ===================
    // Fog
    // ===================

    // set_fog(enabled, start, end) - Configure fog
    engine.register_fn("set_fog", |enabled: bool, start: f64, end: f64| {
        super::push_command(RhaiCommand::SetFog { enabled, start: start as f32, end: end as f32 });
    });

    // enable_fog(start, end)
    engine.register_fn("enable_fog", |start: f64, end: f64| {
        super::push_command(RhaiCommand::SetFog { enabled: true, start: start as f32, end: end as f32 });
    });

    // disable_fog()
    engine.register_fn("disable_fog", || {
        super::push_command(RhaiCommand::SetFog { enabled: false, start: 10.0, end: 100.0 });
    });

    // set_fog_color(r, g, b) - Set fog color
    engine.register_fn("set_fog_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetFogColor { r: r as f32, g: g as f32, b: b as f32 });
    });

    // ===================
    // Exposure
    // ===================

    // set_ev100(value) - Set camera exposure (higher = darker, ~9.7 for outdoor)
    engine.register_fn("set_ev100", |value: f64| {
        super::push_command(RhaiCommand::SetEv100 { value: value as f32 });
    });

    // set_exposure(value) - Alias for set_ev100
    engine.register_fn("set_exposure", |value: f64| {
        super::push_command(RhaiCommand::SetEv100 { value: value as f32 });
    });
}
