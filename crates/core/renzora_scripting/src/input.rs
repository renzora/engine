use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use std::collections::HashMap;

/// Input state resource collected each frame for scripts
#[derive(Resource, Default, Clone)]
pub struct ScriptInput {
    pub keys_pressed: HashMap<KeyCode, bool>,
    pub keys_just_pressed: HashMap<KeyCode, bool>,
    pub keys_just_released: HashMap<KeyCode, bool>,
    pub mouse_pressed: HashMap<MouseButton, bool>,
    pub mouse_just_pressed: HashMap<MouseButton, bool>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub scroll_delta: Vec2,
    pub gamepad_axes: HashMap<u32, HashMap<GamepadAxis, f32>>,
    pub gamepad_buttons: HashMap<u32, HashMap<GamepadButton, bool>>,
}

impl ScriptInput {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn get_movement_vector(&self) -> Vec2 {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) { x -= 1.0; }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) { x += 1.0; }
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) { y -= 1.0; }
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) { y += 1.0; }
        let v = Vec2::new(x, y);
        if v.length_squared() > 0.0 { v.normalize() } else { v }
    }

    pub fn get_gamepad_left_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return Vec2::ZERO };
        Vec2::new(
            axes.get(&GamepadAxis::LeftStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::LeftStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_right_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return Vec2::ZERO };
        Vec2::new(
            axes.get(&GamepadAxis::RightStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::RightStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_trigger(&self, id: u32, left: bool) -> f32 {
        let axes = match self.gamepad_axes.get(&id) { Some(a) => a, None => return 0.0 };
        let axis = if left { GamepadAxis::LeftZ } else { GamepadAxis::RightZ };
        axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn is_gamepad_button_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons.get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }
}

/// System to update ScriptInput from Bevy input resources
pub fn update_script_input(
    mut script_input: ResMut<ScriptInput>,
    mut keyboard_events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    gamepads: Query<(Entity, &Gamepad)>,
) {
    script_input.keys_just_pressed.clear();
    script_input.keys_just_released.clear();
    script_input.mouse_just_pressed.clear();
    script_input.mouse_delta = Vec2::ZERO;
    script_input.scroll_delta = Vec2::ZERO;

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

    for button in mouse_buttons.get_pressed() {
        script_input.mouse_pressed.insert(*button, true);
    }
    for button in mouse_buttons.get_just_pressed() {
        script_input.mouse_just_pressed.insert(*button, true);
    }
    for button in mouse_buttons.get_just_released() {
        script_input.mouse_pressed.remove(button);
    }

    if let Ok(window) = windows.single() {
        if let Some(pos) = window.cursor_position() {
            script_input.mouse_position = pos;
        }
    }

    for event in mouse_motion.read() {
        script_input.mouse_delta += event.delta;
    }
    for event in scroll.read() {
        script_input.scroll_delta += Vec2::new(event.x, event.y);
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
