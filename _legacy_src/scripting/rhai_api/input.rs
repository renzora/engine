//! Input API functions for Rhai scripts
//!
//! These functions check input state that's been pre-populated in the scope.

use rhai::{Dynamic, Engine, Map, ImmutableString};
use crate::core::resources::console::{console_log, LogLevel};

/// Register input functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Keyboard Input
    // ===================

    // is_key_pressed(key) - Check if key is currently held
    // Key names: "W", "A", "S", "D", "Space", "Shift", "Ctrl", "Escape", etc.
    engine.register_fn("is_key_pressed", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str())
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false)
    });

    // is_key_just_pressed(key) - Check if key was pressed this frame
    engine.register_fn("is_key_just_pressed", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str())
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false)
    });

    // is_key_just_released(key) - Check if key was released this frame
    engine.register_fn("is_key_just_released", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str())
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false)
    });

    // ===================
    // Mouse Input
    // ===================

    // get_mouse_position() - Returns map with x, y
    engine.register_fn("get_mouse_position", |mouse_x: f64, mouse_y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(mouse_x));
        m.insert("y".into(), Dynamic::from(mouse_y));
        m
    });

    // get_mouse_delta() - Returns map with x, y for mouse movement
    engine.register_fn("get_mouse_delta", |delta_x: f64, delta_y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(delta_x));
        m.insert("y".into(), Dynamic::from(delta_y));
        m
    });

    // ===================
    // Gamepad Input
    // ===================

    // get_gamepad_axis(axis_name) - Get axis value (-1.0 to 1.0)
    // Axis names: "left_x", "left_y", "right_x", "right_y", "left_trigger", "right_trigger"
    engine.register_fn("get_gamepad_axis", |
        left_x: f64, left_y: f64,
        right_x: f64, right_y: f64,
        left_trigger: f64, right_trigger: f64,
        axis: ImmutableString
    | -> f64 {
        match axis.as_str() {
            "left_x" => left_x,
            "left_y" => left_y,
            "right_x" => right_x,
            "right_y" => right_y,
            "left_trigger" => left_trigger,
            "right_trigger" => right_trigger,
            _ => 0.0,
        }
    });

    // get_left_stick() - Returns map with x, y
    engine.register_fn("get_left_stick", |left_x: f64, left_y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(left_x));
        m.insert("y".into(), Dynamic::from(left_y));
        m
    });

    // get_right_stick() - Returns map with x, y
    engine.register_fn("get_right_stick", |right_x: f64, right_y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(right_x));
        m.insert("y".into(), Dynamic::from(right_y));
        m
    });

    // ===================
    // Input Axis Helpers
    // ===================

    // get_movement_axis() - Returns normalized WASD/arrows movement as map with x, y
    engine.register_fn("get_movement_axis", |input_x: f64, input_y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(input_x));
        m.insert("y".into(), Dynamic::from(input_y));
        m
    });

    // ===================
    // Print for debugging
    // ===================

    // print_log(msg) - Print to console (executes immediately)
    engine.register_fn("print_log", |msg: ImmutableString| {
        console_log(LogLevel::Info, "Script", msg.to_string());
    });
}
