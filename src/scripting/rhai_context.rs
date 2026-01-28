//! Rhai script execution context

use bevy::prelude::*;
use std::collections::HashMap;

use super::{ScriptTime, ScriptTransform, RhaiCommand};

/// Child node info for scripts
#[derive(Clone)]
pub struct ChildNodeInfo {
    pub entity: Entity,
    pub name: String,
    pub position: Vec3,
    pub rotation: Vec3, // euler degrees
    pub scale: Vec3,
}

/// Pending child transform change
#[derive(Clone, Default)]
pub struct ChildChange {
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
}

/// Context passed to Rhai scripts
pub struct RhaiScriptContext {
    // ===================
    // Time
    // ===================
    pub time: ScriptTime,

    // ===================
    // Transform
    // ===================
    pub transform: ScriptTransform,

    // ===================
    // Input - Movement
    // ===================
    pub input_movement: Vec2,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,

    // ===================
    // Input - Keyboard
    // ===================
    pub keys_pressed: HashMap<String, bool>,
    pub keys_just_pressed: HashMap<String, bool>,
    pub keys_just_released: HashMap<String, bool>,

    // ===================
    // Input - Mouse
    // ===================
    pub mouse_buttons_pressed: [bool; 5],
    pub mouse_buttons_just_pressed: [bool; 5],
    pub mouse_scroll: f32,

    // ===================
    // Input - Gamepad
    // ===================
    pub gamepad_left_stick: Vec2,
    pub gamepad_right_stick: Vec2,
    pub gamepad_left_trigger: f32,
    pub gamepad_right_trigger: f32,
    pub gamepad_buttons: [bool; 16],
    pub gamepad_buttons_just_pressed: [bool; 16],

    // ===================
    // Hierarchy - Parent
    // ===================
    pub has_parent: bool,
    pub parent_entity: Option<Entity>,
    pub parent_position: Vec3,
    pub parent_rotation: Vec3,
    pub parent_scale: Vec3,

    // ===================
    // Hierarchy - Children
    // ===================
    pub children: Vec<ChildNodeInfo>,

    // ===================
    // Entity Info
    // ===================
    pub self_entity: Option<Entity>,
    pub self_entity_id: u64,
    pub self_entity_name: String,
    pub found_entities: HashMap<String, u64>,

    // ===================
    // Timer State
    // ===================
    pub active_timers: HashMap<String, TimerState>,
    pub timers_just_finished: Vec<String>,

    // ===================
    // Outputs - Transform
    // ===================
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
    pub rotation_delta: Option<Vec3>,
    pub new_scale: Option<Vec3>,
    pub look_at_target: Option<Vec3>,
    pub print_message: Option<String>,

    // ===================
    // Outputs - Parent Transform
    // ===================
    pub parent_new_position: Option<Vec3>,
    pub parent_new_rotation: Option<Vec3>,
    pub parent_translation: Option<Vec3>,

    // ===================
    // Outputs - Child Transforms
    // ===================
    pub child_changes: HashMap<String, ChildChange>,

    // ===================
    // Outputs - Commands
    // ===================
    pub commands: Vec<RhaiCommand>,

    // ===================
    // Outputs - Environment
    // ===================
    pub env_sky_mode: Option<u8>,  // 0=Color, 1=Procedural, 2=Panorama
    pub env_clear_color: Option<(f32, f32, f32)>,
    pub env_ambient_brightness: Option<f32>,
    pub env_ambient_color: Option<(f32, f32, f32)>,
    pub env_ev100: Option<f32>,
    // Procedural sky
    pub env_sky_top_color: Option<(f32, f32, f32)>,
    pub env_sky_horizon_color: Option<(f32, f32, f32)>,
    pub env_sky_curve: Option<f32>,
    pub env_ground_bottom_color: Option<(f32, f32, f32)>,
    pub env_ground_horizon_color: Option<(f32, f32, f32)>,
    pub env_ground_curve: Option<f32>,
    // Sun
    pub env_sun_azimuth: Option<f32>,
    pub env_sun_elevation: Option<f32>,
    pub env_sun_color: Option<(f32, f32, f32)>,
    pub env_sun_energy: Option<f32>,
    pub env_sun_disk_scale: Option<f32>,
    // Fog
    pub env_fog_enabled: Option<bool>,
    pub env_fog_color: Option<(f32, f32, f32)>,
    pub env_fog_start: Option<f32>,
    pub env_fog_end: Option<f32>,
}

/// Timer state for script-managed timers
#[derive(Clone, Debug)]
pub struct TimerState {
    pub duration: f32,
    pub elapsed: f32,
    pub repeat: bool,
    pub paused: bool,
}

impl RhaiScriptContext {
    pub fn new(time: ScriptTime, transform: ScriptTransform) -> Self {
        Self {
            time,
            transform,
            input_movement: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            keys_pressed: HashMap::new(),
            keys_just_pressed: HashMap::new(),
            keys_just_released: HashMap::new(),
            mouse_buttons_pressed: [false; 5],
            mouse_buttons_just_pressed: [false; 5],
            mouse_scroll: 0.0,
            gamepad_left_stick: Vec2::ZERO,
            gamepad_right_stick: Vec2::ZERO,
            gamepad_left_trigger: 0.0,
            gamepad_right_trigger: 0.0,
            gamepad_buttons: [false; 16],
            gamepad_buttons_just_pressed: [false; 16],
            has_parent: false,
            parent_entity: None,
            parent_position: Vec3::ZERO,
            parent_rotation: Vec3::ZERO,
            parent_scale: Vec3::ONE,
            children: Vec::new(),
            self_entity: None,
            self_entity_id: 0,
            self_entity_name: String::new(),
            found_entities: HashMap::new(),
            active_timers: HashMap::new(),
            timers_just_finished: Vec::new(),
            new_position: None,
            new_rotation: None,
            translation: None,
            rotation_delta: None,
            new_scale: None,
            look_at_target: None,
            print_message: None,
            parent_new_position: None,
            parent_new_rotation: None,
            parent_translation: None,
            child_changes: HashMap::new(),
            commands: Vec::new(),
            env_sky_mode: None,
            env_clear_color: None,
            env_ambient_brightness: None,
            env_ambient_color: None,
            env_ev100: None,
            env_sky_top_color: None,
            env_sky_horizon_color: None,
            env_sky_curve: None,
            env_ground_bottom_color: None,
            env_ground_horizon_color: None,
            env_ground_curve: None,
            env_sun_azimuth: None,
            env_sun_elevation: None,
            env_sun_color: None,
            env_sun_energy: None,
            env_sun_disk_scale: None,
            env_fog_enabled: None,
            env_fog_color: None,
            env_fog_start: None,
            env_fog_end: None,
        }
    }

    /// Take all queued commands
    pub fn take_commands(&mut self) -> Vec<RhaiCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Check if a timer just finished this frame
    pub fn timer_just_finished(&self, name: &str) -> bool {
        self.timers_just_finished.contains(&name.to_string())
    }
}
