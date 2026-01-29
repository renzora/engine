//! Gamepad debug state resource

use bevy::prelude::*;

/// Cached gamepad state for debugging panel
#[derive(Resource, Default)]
pub struct GamepadDebugState {
    pub gamepads: Vec<GamepadInfo>,
}

/// Information about a single gamepad
#[derive(Clone, Default)]
pub struct GamepadInfo {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: GamepadButtonState,
}

/// State of all gamepad buttons
#[derive(Clone, Default)]
pub struct GamepadButtonState {
    pub south: bool,      // A / Cross
    pub east: bool,       // B / Circle
    pub west: bool,       // X / Square
    pub north: bool,      // Y / Triangle
    pub left_trigger: bool,   // LB
    pub right_trigger: bool,  // RB
    pub left_trigger2: bool,  // LT button
    pub right_trigger2: bool, // RT button
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub start: bool,
    pub select: bool,
    pub left_thumb: bool,  // L3
    pub right_thumb: bool, // R3
}

/// System to update gamepad debug state
pub fn update_gamepad_debug_state(
    mut debug_state: ResMut<GamepadDebugState>,
    gamepads: Query<&Gamepad>,
) {
    debug_state.gamepads.clear();

    for gamepad in gamepads.iter() {
        let info = GamepadInfo {
            left_stick: Vec2::new(
                gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0),
                gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0),
            ),
            right_stick: Vec2::new(
                gamepad.get(GamepadAxis::RightStickX).unwrap_or(0.0),
                gamepad.get(GamepadAxis::RightStickY).unwrap_or(0.0),
            ),
            left_trigger: gamepad.get(GamepadAxis::LeftZ).unwrap_or(0.0),
            right_trigger: gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0),
            buttons: GamepadButtonState {
                south: gamepad.pressed(GamepadButton::South),
                east: gamepad.pressed(GamepadButton::East),
                west: gamepad.pressed(GamepadButton::West),
                north: gamepad.pressed(GamepadButton::North),
                left_trigger: gamepad.pressed(GamepadButton::LeftTrigger),
                right_trigger: gamepad.pressed(GamepadButton::RightTrigger),
                left_trigger2: gamepad.pressed(GamepadButton::LeftTrigger2),
                right_trigger2: gamepad.pressed(GamepadButton::RightTrigger2),
                dpad_up: gamepad.pressed(GamepadButton::DPadUp),
                dpad_down: gamepad.pressed(GamepadButton::DPadDown),
                dpad_left: gamepad.pressed(GamepadButton::DPadLeft),
                dpad_right: gamepad.pressed(GamepadButton::DPadRight),
                start: gamepad.pressed(GamepadButton::Start),
                select: gamepad.pressed(GamepadButton::Select),
                left_thumb: gamepad.pressed(GamepadButton::LeftThumb),
                right_thumb: gamepad.pressed(GamepadButton::RightThumb),
            },
        };
        debug_state.gamepads.push(info);
    }
}
