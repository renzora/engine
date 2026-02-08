//! Rendering API functions for Rhai scripts

use rhai::Engine;
use super::super::rhai_commands::RhaiCommand;

/// Register rendering functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Visibility
    // ===================

    // set_visible(visible) - Set self visibility
    engine.register_fn("set_visible", |visible: bool| {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible });
    });

    // set_visible_of(entity_id, visible) - Set entity visibility
    engine.register_fn("set_visible_of", |entity_id: i64, visible: bool| {
        super::push_command(RhaiCommand::SetVisibility { entity_id: Some(entity_id as u64), visible });
    });

    // show() - Make self visible
    engine.register_fn("show", || {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible: true });
    });

    // hide() - Make self invisible
    engine.register_fn("hide", || {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible: false });
    });

    // ===================
    // Material/Color
    // ===================

    // set_color(r, g, b, a) - Set material color of self
    engine.register_fn("set_color", |r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: None, color: [r as f32, g as f32, b as f32, a as f32] });
    });

    // set_color_rgb(r, g, b) - Set material color (full opacity)
    engine.register_fn("set_color_rgb", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: None, color: [r as f32, g as f32, b as f32, 1.0] });
    });

    // set_color_of(entity_id, r, g, b, a)
    engine.register_fn("set_color_of", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: Some(entity_id as u64), color: [r as f32, g as f32, b as f32, a as f32] });
    });

    // set_opacity(alpha) - Set material opacity
    engine.register_fn("set_opacity", |alpha: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: None, color: [1.0, 1.0, 1.0, alpha as f32] });
    });

    // ===================
    // Lights
    // ===================

    // set_light_intensity(intensity)
    engine.register_fn("set_light_intensity", |intensity: f64| {
        super::push_command(RhaiCommand::SetLightIntensity { entity_id: None, intensity: intensity as f32 });
    });

    // set_light_intensity_of(entity_id, intensity)
    engine.register_fn("set_light_intensity_of", |entity_id: i64, intensity: f64| {
        super::push_command(RhaiCommand::SetLightIntensity { entity_id: Some(entity_id as u64), intensity: intensity as f32 });
    });

    // set_light_color(r, g, b)
    engine.register_fn("set_light_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetLightColor { entity_id: None, color: [r as f32, g as f32, b as f32] });
    });

    // set_light_color_of(entity_id, r, g, b)
    engine.register_fn("set_light_color_of", |entity_id: i64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetLightColor { entity_id: Some(entity_id as u64), color: [r as f32, g as f32, b as f32] });
    });

    // set_light_range(range) - For point/spot lights
    engine.register_fn("set_light_range", |_range: f64| {
        // Light range is handled through the component system
    });

    // ===================
    // Sprite
    // ===================

    // set_sprite_color(r, g, b, a)
    engine.register_fn("set_sprite_color", |r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: None, color: [r as f32, g as f32, b as f32, a as f32] });
    });

    // flip_sprite_x(flipped) - placeholder
    engine.register_fn("flip_sprite_x", |_flipped: bool| {});

    // flip_sprite_y(flipped) - placeholder
    engine.register_fn("flip_sprite_y", |_flipped: bool| {});
}
