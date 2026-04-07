use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use std::collections::HashMap;

// Re-export ScriptInput from renzora_core
pub use renzora_core::ScriptInput;

/// System to update ScriptInput from Bevy input resources
pub fn update_script_input(
    mut script_input: ResMut<ScriptInput>,
    keyboard_events: Option<MessageReader<bevy::input::keyboard::KeyboardInput>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window>,
    mouse_motion: Option<MessageReader<bevy::input::mouse::MouseMotion>>,
    scroll: Option<MessageReader<bevy::input::mouse::MouseWheel>>,
    gamepads: Query<(Entity, &Gamepad)>,
) {
    script_input.keys_just_pressed.clear();
    script_input.keys_just_released.clear();
    script_input.mouse_just_pressed.clear();
    script_input.mouse_delta = Vec2::ZERO;
    script_input.scroll_delta = Vec2::ZERO;

    if let Some(mut keyboard_events) = keyboard_events {
        for event in keyboard_events.read() {
            if event.state.is_pressed() {
                if !script_input.keys_pressed.contains_key(&event.key_code) {
                    script_input.keys_just_pressed.insert(event.key_code, true);
                }
                script_input.keys_pressed.insert(event.key_code, true);
            } else {
                script_input.keys_pressed.remove(&event.key_code);
                script_input.keys_just_released.insert(event.key_code, true);
            }
        }
    }

    if let Some(mouse_buttons) = mouse_buttons {
        for button in mouse_buttons.get_pressed() {
            script_input.mouse_pressed.insert(*button, true);
        }
        for button in mouse_buttons.get_just_pressed() {
            script_input.mouse_just_pressed.insert(*button, true);
        }
        for button in mouse_buttons.get_just_released() {
            script_input.mouse_pressed.remove(button);
        }
    }

    if let Ok(window) = windows.single() {
        if let Some(pos) = window.cursor_position() {
            script_input.mouse_position = pos;
        }
    }

    if let Some(mut mouse_motion) = mouse_motion {
        for event in mouse_motion.read() {
            script_input.mouse_delta += event.delta;
        }
    }
    if let Some(mut scroll) = scroll {
        for event in scroll.read() {
            script_input.scroll_delta += Vec2::new(event.x, event.y);
        }
    }

    script_input.gamepad_axes.clear();
    script_input.gamepad_buttons.clear();

    for (gamepad_idx, (_entity, gamepad)) in gamepads.iter().enumerate() {
        let id = gamepad_idx as u32;
        let mut axes = HashMap::new();
        let ls = gamepad.left_stick();
        let rs = gamepad.right_stick();
        axes.insert(GamepadAxis::LeftStickX, ls.x);
        axes.insert(GamepadAxis::LeftStickY, ls.y);
        axes.insert(GamepadAxis::RightStickX, rs.x);
        axes.insert(GamepadAxis::RightStickY, rs.y);
        axes.insert(GamepadAxis::LeftZ, gamepad.get(GamepadAxis::LeftZ).unwrap_or(0.0));
        axes.insert(GamepadAxis::RightZ, gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0));
        script_input.gamepad_axes.insert(id, axes);

        let mut buttons = HashMap::new();
        for btn in [
            GamepadButton::South, GamepadButton::East, GamepadButton::West, GamepadButton::North,
            GamepadButton::LeftTrigger, GamepadButton::RightTrigger,
            GamepadButton::LeftTrigger2, GamepadButton::RightTrigger2,
            GamepadButton::Select, GamepadButton::Start,
            GamepadButton::LeftThumb, GamepadButton::RightThumb,
            GamepadButton::DPadUp, GamepadButton::DPadDown,
            GamepadButton::DPadLeft, GamepadButton::DPadRight,
        ] {
            buttons.insert(btn, gamepad.pressed(btn));
        }
        script_input.gamepad_buttons.insert(id, buttons);
    }
}
