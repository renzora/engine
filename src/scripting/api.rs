//! Scripting API - exposes game values to scripts
//!
//! This module provides the types and functions available to game scripts.

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use std::collections::HashMap;

/// Input state available to scripts
#[derive(Resource, Default, Clone)]
pub struct ScriptInput {
    /// Keys currently held down
    pub keys_pressed: HashMap<KeyCode, bool>,
    /// Keys just pressed this frame
    pub keys_just_pressed: HashMap<KeyCode, bool>,
    /// Keys just released this frame
    pub keys_just_released: HashMap<KeyCode, bool>,
    /// Mouse buttons currently held
    pub mouse_pressed: HashMap<MouseButton, bool>,
    /// Mouse buttons just pressed this frame
    pub mouse_just_pressed: HashMap<MouseButton, bool>,
    /// Mouse position in window coordinates
    pub mouse_position: Vec2,
    /// Mouse motion delta this frame
    pub mouse_delta: Vec2,
    /// Mouse scroll delta this frame
    pub scroll_delta: Vec2,
    /// Gamepad axis values (gamepad_id -> axis -> value)
    pub gamepad_axes: HashMap<u32, HashMap<GamepadAxis, f32>>,
    /// Gamepad buttons pressed
    pub gamepad_buttons: HashMap<u32, HashMap<GamepadButton, bool>>,
}

impl ScriptInput {
    /// Check if a key is currently pressed
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.get(&key).copied().unwrap_or(false)
    }

    /// Check if a key was just pressed this frame
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.get(&key).copied().unwrap_or(false)
    }

    /// Check if a key was just released this frame
    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.keys_just_released.get(&key).copied().unwrap_or(false)
    }

    /// Check if a mouse button is pressed
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.get(&button).copied().unwrap_or(false)
    }

    /// Check if a mouse button was just pressed
    pub fn is_mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.get(&button).copied().unwrap_or(false)
    }

    /// Get the horizontal input axis (A/D or Left/Right arrows)
    pub fn get_axis_horizontal(&self) -> f32 {
        let mut value = 0.0;
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) {
            value -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) {
            value += 1.0;
        }
        value
    }

    /// Get the vertical input axis (W/S or Up/Down arrows)
    pub fn get_axis_vertical(&self) -> f32 {
        let mut value = 0.0;
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) {
            value -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) {
            value += 1.0;
        }
        value
    }

    /// Get movement vector from WASD/Arrow keys (normalized)
    pub fn get_movement_vector(&self) -> Vec2 {
        let vec = Vec2::new(self.get_axis_horizontal(), self.get_axis_vertical());
        if vec.length_squared() > 0.0 {
            vec.normalize()
        } else {
            vec
        }
    }

    /// Get gamepad left stick X axis (-1.0 to 1.0)
    pub fn get_gamepad_left_stick_x(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::LeftStickX))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get gamepad left stick Y axis (-1.0 to 1.0)
    pub fn get_gamepad_left_stick_y(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::LeftStickY))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get gamepad right stick X axis (-1.0 to 1.0)
    pub fn get_gamepad_right_stick_x(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::RightStickX))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get gamepad right stick Y axis (-1.0 to 1.0)
    pub fn get_gamepad_right_stick_y(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::RightStickY))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get gamepad left trigger (0.0 to 1.0)
    pub fn get_gamepad_left_trigger(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::LeftZ))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get gamepad right trigger (0.0 to 1.0)
    pub fn get_gamepad_right_trigger(&self, gamepad_id: u32) -> f32 {
        self.gamepad_axes
            .get(&gamepad_id)
            .and_then(|axes| axes.get(&GamepadAxis::RightZ))
            .copied()
            .unwrap_or(0.0)
    }

    /// Check if a gamepad button is pressed
    pub fn is_gamepad_button_pressed(&self, gamepad_id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons
            .get(&gamepad_id)
            .and_then(|buttons| buttons.get(&button))
            .copied()
            .unwrap_or(false)
    }
}

/// Time information available to scripts
#[derive(Clone, Copy)]
pub struct ScriptTime {
    /// Time since startup in seconds
    pub elapsed: f64,
    /// Delta time since last frame in seconds
    pub delta: f32,
    /// Fixed delta time for physics (usually 1/60)
    pub fixed_delta: f32,
    /// Current frame number
    pub frame_count: u64,
}

impl Default for ScriptTime {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            delta: 1.0 / 60.0,
            fixed_delta: 1.0 / 60.0,
            frame_count: 0,
        }
    }
}

/// Transform wrapper for scripts with convenient methods
#[derive(Clone, Copy)]
pub struct ScriptTransform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl ScriptTransform {
    pub fn from_transform(t: &Transform) -> Self {
        Self {
            position: t.translation,
            rotation: t.rotation,
            scale: t.scale,
        }
    }

    /// Get the forward direction (-Z in Bevy's coordinate system)
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Get the right direction (+X)
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Get the up direction (+Y)
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Get euler angles (pitch, yaw, roll) in radians
    pub fn euler_angles(&self) -> Vec3 {
        let (x, y, z) = self.rotation.to_euler(EulerRot::XYZ);
        Vec3::new(x, y, z)
    }

    /// Get euler angles in degrees
    pub fn euler_angles_degrees(&self) -> Vec3 {
        self.euler_angles() * (180.0 / std::f32::consts::PI)
    }

    /// Create a look-at rotation
    pub fn look_at(&self, target: Vec3, _up: Vec3) -> Quat {
        let forward = (target - self.position).normalize();
        Quat::from_rotation_arc(Vec3::NEG_Z, forward)
    }
}

impl Default for ScriptTransform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// Sky/environment data available to scripts
#[derive(Clone)]
pub struct ScriptEnvironment {
    pub ambient_color: Vec3,
    pub ambient_brightness: f32,
    pub clear_color: Vec3,
    pub fog_enabled: bool,
    pub fog_color: Vec3,
    pub fog_start: f32,
    pub fog_end: f32,
}

impl Default for ScriptEnvironment {
    fn default() -> Self {
        Self {
            ambient_color: Vec3::ONE,
            ambient_brightness: 300.0,
            clear_color: Vec3::new(0.4, 0.6, 0.9),
            fog_enabled: false,
            fog_color: Vec3::splat(0.5),
            fog_start: 10.0,
            fog_end: 100.0,
        }
    }
}

/// Context passed to scripts containing all game state
pub struct ScriptContext<'a> {
    /// The entity this script is attached to
    pub entity: Entity,
    /// This entity's transform
    pub transform: ScriptTransform,
    /// Time information
    pub time: ScriptTime,
    /// Input state
    pub input: &'a ScriptInput,
    /// Environment/sky settings
    pub environment: ScriptEnvironment,
    /// Commands for spawning/modifying entities (queued)
    commands: Vec<ScriptCommand>,
}

impl<'a> ScriptContext<'a> {
    pub fn new(entity: Entity, transform: ScriptTransform, time: ScriptTime, input: &'a ScriptInput) -> Self {
        Self {
            entity,
            transform,
            time,
            input,
            environment: ScriptEnvironment::default(),
            commands: Vec::new(),
        }
    }

    // --- Transform modifications ---

    /// Move the entity by a delta
    pub fn translate(&mut self, delta: Vec3) {
        self.commands.push(ScriptCommand::Translate { entity: self.entity, delta });
    }

    /// Move in local space (relative to current rotation)
    pub fn translate_local(&mut self, delta: Vec3) {
        let world_delta = self.transform.rotation * delta;
        self.translate(world_delta);
    }

    /// Set absolute position
    pub fn set_position(&mut self, position: Vec3) {
        self.commands.push(ScriptCommand::SetPosition { entity: self.entity, position });
    }

    /// Rotate by euler angles (in radians)
    pub fn rotate_euler(&mut self, euler: Vec3) {
        let rotation = Quat::from_euler(EulerRot::XYZ, euler.x, euler.y, euler.z);
        self.commands.push(ScriptCommand::Rotate { entity: self.entity, rotation });
    }

    /// Rotate by euler angles (in degrees)
    pub fn rotate_degrees(&mut self, degrees: Vec3) {
        let radians = degrees * (std::f32::consts::PI / 180.0);
        self.rotate_euler(radians);
    }

    /// Set absolute rotation
    pub fn set_rotation(&mut self, rotation: Quat) {
        self.commands.push(ScriptCommand::SetRotation { entity: self.entity, rotation });
    }

    /// Look at a target position
    pub fn look_at(&mut self, target: Vec3) {
        self.commands.push(ScriptCommand::LookAt { entity: self.entity, target });
    }

    /// Set scale
    pub fn set_scale(&mut self, scale: Vec3) {
        self.commands.push(ScriptCommand::SetScale { entity: self.entity, scale });
    }

    // --- Logging ---

    /// Print a message to the console
    pub fn print(&mut self, message: &str) {
        self.commands.push(ScriptCommand::Print { message: message.to_string() });
    }

    // --- Entity commands ---

    /// Spawn a new entity (returns a placeholder ID, actual entity created later)
    pub fn spawn(&mut self, name: &str) -> u64 {
        let id = self.commands.len() as u64;
        self.commands.push(ScriptCommand::Spawn { name: name.to_string() });
        id
    }

    /// Destroy this entity
    pub fn destroy_self(&mut self) {
        self.commands.push(ScriptCommand::Destroy { entity: self.entity });
    }

    /// Take queued commands
    pub fn take_commands(&mut self) -> Vec<ScriptCommand> {
        std::mem::take(&mut self.commands)
    }
}

/// Commands queued by scripts to be executed after script runs
#[derive(Clone, Debug)]
pub enum ScriptCommand {
    Translate { entity: Entity, delta: Vec3 },
    SetPosition { entity: Entity, position: Vec3 },
    Rotate { entity: Entity, rotation: Quat },
    SetRotation { entity: Entity, rotation: Quat },
    LookAt { entity: Entity, target: Vec3 },
    SetScale { entity: Entity, scale: Vec3 },
    Print { message: String },
    Spawn { name: String },
    Destroy { entity: Entity },
}

/// System to update ScriptInput from Bevy's input resources
pub fn update_script_input(
    mut script_input: ResMut<ScriptInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    gamepads: Query<(Entity, &Gamepad)>,
) {
    // Clear just pressed/released
    script_input.keys_just_pressed.clear();
    script_input.keys_just_released.clear();
    script_input.mouse_just_pressed.clear();
    script_input.mouse_delta = Vec2::ZERO;
    script_input.scroll_delta = Vec2::ZERO;

    // Update keyboard state
    for key in keyboard.get_pressed() {
        script_input.keys_pressed.insert(*key, true);
    }
    for key in keyboard.get_just_pressed() {
        script_input.keys_just_pressed.insert(*key, true);
    }
    for key in keyboard.get_just_released() {
        script_input.keys_pressed.remove(key);
        script_input.keys_just_released.insert(*key, true);
    }

    // Update mouse buttons
    for button in mouse_buttons.get_pressed() {
        script_input.mouse_pressed.insert(*button, true);
    }
    for button in mouse_buttons.get_just_pressed() {
        script_input.mouse_just_pressed.insert(*button, true);
    }
    for button in mouse_buttons.get_just_released() {
        script_input.mouse_pressed.remove(button);
    }

    // Update mouse position
    if let Ok(window) = windows.single() {
        if let Some(pos) = window.cursor_position() {
            script_input.mouse_position = pos;
        }
    }

    // Update mouse motion
    for event in mouse_motion.read() {
        script_input.mouse_delta += event.delta;
    }

    // Update scroll
    for event in scroll.read() {
        script_input.scroll_delta += Vec2::new(event.x, event.y);
    }

    // Update gamepad input
    script_input.gamepad_axes.clear();
    script_input.gamepad_buttons.clear();

    let gamepad_count = gamepads.iter().count();
    if gamepad_count > 0 {
        // Log once when gamepads are first detected (use static to track)
        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            info!("[Scripting] Detected {} gamepad(s)", gamepad_count);
        }
    }

    for (gamepad_idx, (_entity, gamepad)) in gamepads.iter().enumerate() {
        let gamepad_id = gamepad_idx as u32;

        // Get axis values using Bevy 0.17 convenience methods
        let mut axes = HashMap::new();
        let left_stick = gamepad.left_stick();
        let right_stick = gamepad.right_stick();
        axes.insert(GamepadAxis::LeftStickX, left_stick.x);
        axes.insert(GamepadAxis::LeftStickY, left_stick.y);
        axes.insert(GamepadAxis::RightStickX, right_stick.x);
        axes.insert(GamepadAxis::RightStickY, right_stick.y);
        // Triggers: read as axes (LeftZ/RightZ) for PS5 and most controllers
        let left_trigger = gamepad.get(GamepadAxis::LeftZ).unwrap_or(0.0);
        let right_trigger = gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0);
        axes.insert(GamepadAxis::LeftZ, left_trigger);
        axes.insert(GamepadAxis::RightZ, right_trigger);
        script_input.gamepad_axes.insert(gamepad_id, axes);

        // Get button states
        let mut buttons = HashMap::new();
        let button_types = [
            GamepadButton::South,
            GamepadButton::East,
            GamepadButton::West,
            GamepadButton::North,
            GamepadButton::LeftTrigger,
            GamepadButton::RightTrigger,
            GamepadButton::LeftTrigger2,
            GamepadButton::RightTrigger2,
            GamepadButton::Select,
            GamepadButton::Start,
            GamepadButton::LeftThumb,
            GamepadButton::RightThumb,
            GamepadButton::DPadUp,
            GamepadButton::DPadDown,
            GamepadButton::DPadLeft,
            GamepadButton::DPadRight,
        ];
        for button in button_types {
            buttons.insert(button, gamepad.pressed(button));
        }
        script_input.gamepad_buttons.insert(gamepad_id, buttons);
    }
}
