use bevy::prelude::*;
use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use serde::{Deserialize, Serialize};

/// What kind of value an action produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum ActionKind {
    /// Pressed / not pressed.
    Button,
    /// Single float axis (-1.0 to 1.0).
    Axis1D,
    /// Two-axis vector (e.g. movement stick).
    Axis2D,
}

/// A physical input that can be bound to an action.
///
/// Uses string representations for Bevy types so it can be serialized to RON/JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub enum InputBinding {
    /// A keyboard key (stored as Debug string, e.g. "KeyW", "Space").
    Key(String),
    /// A mouse button ("Left", "Right", "Middle").
    MouseButton(String),
    /// A gamepad button ("South", "East", "West", "North", etc.).
    GamepadButton(String),
    /// A single gamepad axis ("LeftStickX", "RightStickY", etc.).
    GamepadAxis(String),
    /// Four keys composited into a 2D axis.
    Composite2D {
        up: String,
        down: String,
        left: String,
        right: String,
    },
}

impl InputBinding {
    pub fn key(key: KeyCode) -> Self {
        Self::Key(format!("{:?}", key))
    }

    pub fn mouse(button: bevy::prelude::MouseButton) -> Self {
        Self::MouseButton(format!("{:?}", button))
    }

    pub fn gamepad_button(button: GamepadButton) -> Self {
        Self::GamepadButton(format!("{:?}", button))
    }

    pub fn gamepad_axis(axis: GamepadAxis) -> Self {
        Self::GamepadAxis(format!("{:?}", axis))
    }

    pub fn composite_2d(up: KeyCode, down: KeyCode, left: KeyCode, right: KeyCode) -> Self {
        Self::Composite2D {
            up: format!("{:?}", up),
            down: format!("{:?}", down),
            left: format!("{:?}", left),
            right: format!("{:?}", right),
        }
    }

    /// Resolve the key string back to a KeyCode.
    pub fn resolve_key(s: &str) -> Option<KeyCode> {
        key_from_str(s)
    }

    /// Resolve the mouse button string back to a MouseButton.
    pub fn resolve_mouse(s: &str) -> Option<bevy::prelude::MouseButton> {
        match s {
            "Left" => Some(bevy::prelude::MouseButton::Left),
            "Right" => Some(bevy::prelude::MouseButton::Right),
            "Middle" => Some(bevy::prelude::MouseButton::Middle),
            _ => None,
        }
    }

    /// Resolve the gamepad button string back to a GamepadButton.
    pub fn resolve_gamepad_button(s: &str) -> Option<GamepadButton> {
        match s {
            "South" => Some(GamepadButton::South),
            "East" => Some(GamepadButton::East),
            "West" => Some(GamepadButton::West),
            "North" => Some(GamepadButton::North),
            "LeftTrigger" => Some(GamepadButton::LeftTrigger),
            "RightTrigger" => Some(GamepadButton::RightTrigger),
            "LeftTrigger2" => Some(GamepadButton::LeftTrigger2),
            "RightTrigger2" => Some(GamepadButton::RightTrigger2),
            "Select" => Some(GamepadButton::Select),
            "Start" => Some(GamepadButton::Start),
            "LeftThumb" => Some(GamepadButton::LeftThumb),
            "RightThumb" => Some(GamepadButton::RightThumb),
            "DPadUp" => Some(GamepadButton::DPadUp),
            "DPadDown" => Some(GamepadButton::DPadDown),
            "DPadLeft" => Some(GamepadButton::DPadLeft),
            "DPadRight" => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }

    /// Resolve the gamepad axis string back to a GamepadAxis.
    pub fn resolve_gamepad_axis(s: &str) -> Option<GamepadAxis> {
        match s {
            "LeftStickX" => Some(GamepadAxis::LeftStickX),
            "LeftStickY" => Some(GamepadAxis::LeftStickY),
            "RightStickX" => Some(GamepadAxis::RightStickX),
            "RightStickY" => Some(GamepadAxis::RightStickY),
            "LeftZ" => Some(GamepadAxis::LeftZ),
            "RightZ" => Some(GamepadAxis::RightZ),
            _ => None,
        }
    }
}

/// A named input action with one or more bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct InputAction {
    /// Unique name (e.g. "move", "jump", "sprint").
    pub name: String,
    /// What kind of value this action produces.
    pub kind: ActionKind,
    /// Physical inputs bound to this action.
    pub bindings: Vec<InputBinding>,
    /// Dead zone for analog inputs (0.0–1.0).
    pub dead_zone: f32,
}

impl InputAction {
    /// Create a new button action.
    pub fn button(name: impl Into<String>, bindings: Vec<InputBinding>) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Button,
            bindings,
            dead_zone: 0.0,
        }
    }

    /// Create a new 2D axis action.
    pub fn axis_2d(name: impl Into<String>, bindings: Vec<InputBinding>, dead_zone: f32) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Axis2D,
            bindings,
            dead_zone,
        }
    }

    /// Create a new 1D axis action.
    pub fn axis_1d(name: impl Into<String>, bindings: Vec<InputBinding>, dead_zone: f32) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Axis1D,
            bindings,
            dead_zone,
        }
    }
}

/// Parse a KeyCode from its Debug string representation.
fn key_from_str(s: &str) -> Option<KeyCode> {
    Some(match s {
        "KeyA" => KeyCode::KeyA, "KeyB" => KeyCode::KeyB, "KeyC" => KeyCode::KeyC,
        "KeyD" => KeyCode::KeyD, "KeyE" => KeyCode::KeyE, "KeyF" => KeyCode::KeyF,
        "KeyG" => KeyCode::KeyG, "KeyH" => KeyCode::KeyH, "KeyI" => KeyCode::KeyI,
        "KeyJ" => KeyCode::KeyJ, "KeyK" => KeyCode::KeyK, "KeyL" => KeyCode::KeyL,
        "KeyM" => KeyCode::KeyM, "KeyN" => KeyCode::KeyN, "KeyO" => KeyCode::KeyO,
        "KeyP" => KeyCode::KeyP, "KeyQ" => KeyCode::KeyQ, "KeyR" => KeyCode::KeyR,
        "KeyS" => KeyCode::KeyS, "KeyT" => KeyCode::KeyT, "KeyU" => KeyCode::KeyU,
        "KeyV" => KeyCode::KeyV, "KeyW" => KeyCode::KeyW, "KeyX" => KeyCode::KeyX,
        "KeyY" => KeyCode::KeyY, "KeyZ" => KeyCode::KeyZ,
        "Digit0" => KeyCode::Digit0, "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2, "Digit3" => KeyCode::Digit3,
        "Digit4" => KeyCode::Digit4, "Digit5" => KeyCode::Digit5,
        "Digit6" => KeyCode::Digit6, "Digit7" => KeyCode::Digit7,
        "Digit8" => KeyCode::Digit8, "Digit9" => KeyCode::Digit9,
        "Space" => KeyCode::Space,
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "ShiftLeft" => KeyCode::ShiftLeft, "ShiftRight" => KeyCode::ShiftRight,
        "ControlLeft" => KeyCode::ControlLeft, "ControlRight" => KeyCode::ControlRight,
        "AltLeft" => KeyCode::AltLeft, "AltRight" => KeyCode::AltRight,
        "ArrowUp" => KeyCode::ArrowUp, "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft, "ArrowRight" => KeyCode::ArrowRight,
        "F1" => KeyCode::F1, "F2" => KeyCode::F2, "F3" => KeyCode::F3,
        "F4" => KeyCode::F4, "F5" => KeyCode::F5, "F6" => KeyCode::F6,
        "F7" => KeyCode::F7, "F8" => KeyCode::F8, "F9" => KeyCode::F9,
        "F10" => KeyCode::F10, "F11" => KeyCode::F11, "F12" => KeyCode::F12,
        _ => return None,
    })
}
