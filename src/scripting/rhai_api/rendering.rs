//! Rendering API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map};

/// Register rendering functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Visibility
    // ===================

    // set_visible(visible) - Set self visibility
    engine.register_fn("set_visible", |visible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(visible));
        m
    });

    // set_visible_of(entity_id, visible) - Set entity visibility
    engine.register_fn("set_visible_of", |entity_id: i64, visible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("visible".into(), Dynamic::from(visible));
        m
    });

    // show() - Make self visible
    engine.register_fn("show", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(true));
        m
    });

    // hide() - Make self invisible
    engine.register_fn("hide", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(false));
        m
    });

    // ===================
    // Material/Color
    // ===================

    // set_color(r, g, b, a) - Set material color of self
    engine.register_fn("set_color", |r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // set_color_rgb(r, g, b) - Set material color (full opacity)
    engine.register_fn("set_color_rgb", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(1.0));
        m
    });

    // set_color_of(entity_id, r, g, b, a)
    engine.register_fn("set_color_of", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_color"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // set_opacity(alpha) - Set material opacity
    engine.register_fn("set_opacity", |alpha: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_opacity"));
        m.insert("alpha".into(), Dynamic::from(alpha));
        m
    });

    // ===================
    // Lights
    // ===================

    // set_light_intensity(intensity)
    engine.register_fn("set_light_intensity", |intensity: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_intensity"));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m
    });

    // set_light_intensity_of(entity_id, intensity)
    engine.register_fn("set_light_intensity_of", |entity_id: i64, intensity: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_intensity"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m
    });

    // set_light_color(r, g, b)
    engine.register_fn("set_light_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // set_light_color_of(entity_id, r, g, b)
    engine.register_fn("set_light_color_of", |entity_id: i64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_color"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // set_light_range(range) - For point/spot lights
    engine.register_fn("set_light_range", |range: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_range"));
        m.insert("range".into(), Dynamic::from(range));
        m
    });

    // ===================
    // Sprite
    // ===================

    // set_sprite_color(r, g, b, a)
    engine.register_fn("set_sprite_color", |r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_sprite_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // flip_sprite_x(flipped)
    engine.register_fn("flip_sprite_x", |flipped: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("flip_sprite"));
        m.insert("flip_x".into(), Dynamic::from(flipped));
        m
    });

    // flip_sprite_y(flipped)
    engine.register_fn("flip_sprite_y", |flipped: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("flip_sprite"));
        m.insert("flip_y".into(), Dynamic::from(flipped));
        m
    });
}
