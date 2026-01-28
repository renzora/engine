//! Animation API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register animation functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Skeletal Animation
    // ===================

    // play_animation(name) - Play animation on self
    engine.register_fn("play_animation", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_animation"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(true));
        m.insert("speed".into(), Dynamic::from(1.0));
        m
    });

    // play_animation_once(name) - Play animation once (no loop)
    engine.register_fn("play_animation_once", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_animation"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(false));
        m.insert("speed".into(), Dynamic::from(1.0));
        m
    });

    // play_animation_speed(name, speed) - Play animation with custom speed
    engine.register_fn("play_animation_speed", |name: ImmutableString, speed: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_animation"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(true));
        m.insert("speed".into(), Dynamic::from(speed));
        m
    });

    // play_animation_on(entity_id, name)
    engine.register_fn("play_animation_on", |entity_id: i64, name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_animation"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(true));
        m.insert("speed".into(), Dynamic::from(1.0));
        m
    });

    // stop_animation() - Stop animation on self
    engine.register_fn("stop_animation", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_animation"));
        m
    });

    // stop_animation_on(entity_id)
    engine.register_fn("stop_animation_on", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_animation"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // pause_animation()
    engine.register_fn("pause_animation", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("pause_animation"));
        m
    });

    // resume_animation()
    engine.register_fn("resume_animation", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("resume_animation"));
        m
    });

    // set_animation_speed(speed)
    engine.register_fn("set_animation_speed", |speed: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_animation_speed"));
        m.insert("speed".into(), Dynamic::from(speed));
        m
    });

    // ===================
    // Sprite Animation
    // ===================

    // play_sprite_animation(name)
    engine.register_fn("play_sprite_animation", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sprite_animation"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(true));
        m
    });

    // play_sprite_animation_once(name)
    engine.register_fn("play_sprite_animation_once", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sprite_animation"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("looping".into(), Dynamic::from(false));
        m
    });

    // set_sprite_frame(frame_index)
    engine.register_fn("set_sprite_frame", |frame: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_sprite_frame"));
        m.insert("frame".into(), Dynamic::from(frame));
        m
    });

    // ===================
    // Tweening (value interpolation)
    // ===================

    // tween_to(property, target_value, duration, easing)
    // Properties: "position_x", "position_y", "position_z", "rotation_y", "scale", "opacity"
    engine.register_fn("tween_to", |property: ImmutableString, target: f64, duration: f64, easing: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("tween"));
        m.insert("property".into(), Dynamic::from(property));
        m.insert("target".into(), Dynamic::from(target));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("easing".into(), Dynamic::from(easing));
        m
    });

    // tween_position(x, y, z, duration, easing)
    engine.register_fn("tween_position", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("tween_position"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("easing".into(), Dynamic::from(easing));
        m
    });

    // tween_rotation(x, y, z, duration, easing) - degrees
    engine.register_fn("tween_rotation", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("tween_rotation"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("easing".into(), Dynamic::from(easing));
        m
    });

    // tween_scale(x, y, z, duration, easing)
    engine.register_fn("tween_scale", |x: f64, y: f64, z: f64, duration: f64, easing: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("tween_scale"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("duration".into(), Dynamic::from(duration));
        m.insert("easing".into(), Dynamic::from(easing));
        m
    });

    // Easing functions available: "linear", "ease_in", "ease_out", "ease_in_out",
    // "bounce", "elastic", "back", "sine", "quad", "cubic", "quart", "quint", "expo"
}
