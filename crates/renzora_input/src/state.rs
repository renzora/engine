use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis};

use crate::action::{ActionKind, InputBinding};
use crate::map::InputMap;

// Re-export from renzora
pub use renzora::{ActionData, ActionState};

/// System that computes `ActionState` from `InputMap` + raw Bevy input each frame.
pub fn update_action_state(
    input_map: Res<InputMap>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut action_state: ResMut<ActionState>,
) {
    action_state.actions.clear();

    // Grab first gamepad if any
    let gamepad = gamepads.iter().next();

    for action in &input_map.actions {
        let mut data = ActionData::default();

        match action.kind {
            ActionKind::Button => {
                for binding in &action.bindings {
                    match binding {
                        InputBinding::Key(key_str) => {
                            if let Some(key) = InputBinding::resolve_key(key_str) {
                                data.pressed |= keyboard.pressed(key);
                                data.just_pressed |= keyboard.just_pressed(key);
                                data.just_released |= keyboard.just_released(key);
                            }
                        }
                        InputBinding::MouseButton(btn_str) => {
                            if let Some(btn) = InputBinding::resolve_mouse(btn_str) {
                                data.pressed |= mouse_buttons.pressed(btn);
                                data.just_pressed |= mouse_buttons.just_pressed(btn);
                                data.just_released |= mouse_buttons.just_released(btn);
                            }
                        }
                        InputBinding::GamepadButton(btn_str) => {
                            if let Some(gp) = gamepad {
                                if let Some(btn) = InputBinding::resolve_gamepad_button(btn_str) {
                                    data.pressed |= gp.pressed(btn);
                                    data.just_pressed |= gp.just_pressed(btn);
                                    data.just_released |= gp.just_released(btn);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ActionKind::Axis1D => {
                let mut value = 0.0f32;
                for binding in &action.bindings {
                    match binding {
                        InputBinding::Key(key_str) => {
                            if let Some(key) = InputBinding::resolve_key(key_str) {
                                if keyboard.pressed(key) {
                                    value = value.max(1.0);
                                }
                            }
                        }
                        InputBinding::GamepadButton(btn_str) => {
                            // Gamepad buttons (incl. RightTrigger2 / LeftTrigger2
                            // when the pad exposes triggers as digital) contribute
                            // a full 1.0 to the axis when pressed.
                            if let Some(gp) = gamepad {
                                if let Some(btn) = InputBinding::resolve_gamepad_button(btn_str) {
                                    if gp.pressed(btn) {
                                        value = value.max(1.0);
                                    }
                                }
                            }
                        }
                        InputBinding::GamepadAxis(axis_str) => {
                            if let Some(gp) = gamepad {
                                if let Some(axis) = InputBinding::resolve_gamepad_axis(axis_str) {
                                    let v = gp.get(axis).unwrap_or(0.0);
                                    if v.abs() > action.dead_zone {
                                        value += v;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                data.axis_1d = value.clamp(-1.0, 1.0);
                data.pressed = data.axis_1d.abs() > 0.0;
            }
            ActionKind::Axis2D => {
                let mut combined = Vec2::ZERO;
                for binding in &action.bindings {
                    match binding {
                        InputBinding::Composite2D { up, down, left, right } => {
                            let mut v = Vec2::ZERO;
                            if let Some(k) = InputBinding::resolve_key(up) {
                                if keyboard.pressed(k) { v.y += 1.0; }
                            }
                            if let Some(k) = InputBinding::resolve_key(down) {
                                if keyboard.pressed(k) { v.y -= 1.0; }
                            }
                            if let Some(k) = InputBinding::resolve_key(left) {
                                if keyboard.pressed(k) { v.x -= 1.0; }
                            }
                            if let Some(k) = InputBinding::resolve_key(right) {
                                if keyboard.pressed(k) { v.x += 1.0; }
                            }
                            if v.length_squared() > 0.0 {
                                combined += v.normalize();
                            }
                        }
                        InputBinding::GamepadAxis(axis_str) => {
                            if let Some(gp) = gamepad {
                                if let Some(axis) = InputBinding::resolve_gamepad_axis(axis_str) {
                                    let stick = match axis {
                                        GamepadAxis::LeftStickX | GamepadAxis::LeftStickY => {
                                            gp.left_stick()
                                        }
                                        GamepadAxis::RightStickX | GamepadAxis::RightStickY => {
                                            gp.right_stick()
                                        }
                                        _ => Vec2::ZERO,
                                    };
                                    if stick.length() > action.dead_zone {
                                        combined += stick;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                // Clamp to unit circle
                if combined.length() > 1.0 {
                    combined = combined.normalize();
                }
                data.axis_2d = combined;
                data.pressed = combined.length_squared() > 0.0;
            }
        }

        action_state.actions.insert(action.name.clone(), data);
    }
}
