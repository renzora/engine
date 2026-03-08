use bevy::prelude::*;
use std::collections::HashMap;

use crate::command::ScriptCommand;

/// Time info provided to scripts
#[derive(Clone, Copy, Default)]
pub struct ScriptTime {
    pub elapsed: f64,
    pub delta: f32,
    pub fixed_delta: f32,
    pub frame_count: u64,
}

/// Transform wrapper for scripts
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

    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    pub fn euler_degrees(&self) -> Vec3 {
        let (x, y, z) = self.rotation.to_euler(EulerRot::XYZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
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

/// Child node info
#[derive(Clone)]
pub struct ChildNodeInfo {
    pub entity: Entity,
    pub name: String,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

/// Pending child transform change
#[derive(Clone, Default)]
pub struct ChildChange {
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
}

/// Raycast hit result
#[derive(Clone, Debug, Default)]
pub struct RaycastHit {
    pub hit: bool,
    pub entity: Option<Entity>,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

/// Language-agnostic context passed to script backends for execution.
/// Contains all input state and collects all output commands.
pub struct ScriptContext {
    // === Input state ===
    pub time: ScriptTime,
    pub transform: ScriptTransform,

    // Input
    pub input_movement: Vec2,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub camera_yaw: f32,
    pub keys_pressed: HashMap<String, bool>,
    pub keys_just_pressed: HashMap<String, bool>,
    pub keys_just_released: HashMap<String, bool>,
    pub mouse_buttons_pressed: [bool; 5],
    pub mouse_buttons_just_pressed: [bool; 5],
    pub mouse_scroll: f32,

    // Gamepad
    pub gamepad_left_stick: Vec2,
    pub gamepad_right_stick: Vec2,
    pub gamepad_left_trigger: f32,
    pub gamepad_right_trigger: f32,
    pub gamepad_buttons: [bool; 16],
    pub gamepad_buttons_just_pressed: [bool; 16],

    // Hierarchy
    pub has_parent: bool,
    pub parent_entity: Option<Entity>,
    pub parent_position: Vec3,
    pub parent_rotation: Vec3,
    pub parent_scale: Vec3,
    pub children: Vec<ChildNodeInfo>,

    // Entity info
    pub self_entity: Option<Entity>,
    pub self_entity_id: u64,
    pub self_entity_name: String,
    pub found_entities: HashMap<String, u64>,
    pub entities_by_tag: HashMap<String, Vec<u64>>,

    // Collisions
    pub collisions_entered: Vec<u64>,
    pub collisions_exited: Vec<u64>,
    pub active_collisions: Vec<u64>,

    // Timers
    pub timers_just_finished: Vec<String>,

    // Raycasts
    pub raycast_results: HashMap<String, RaycastHit>,

    // Component data
    pub self_health: f32,
    pub self_max_health: f32,
    pub self_health_percent: f32,
    pub self_is_invincible: bool,
    pub self_light_intensity: f32,
    pub self_light_color: [f32; 3],
    pub self_material_color: [f32; 4],

    // === Outputs ===
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
    pub rotation_delta: Option<Vec3>,
    pub new_scale: Option<Vec3>,
    pub look_at_target: Option<Vec3>,

    pub parent_new_position: Option<Vec3>,
    pub parent_new_rotation: Option<Vec3>,
    pub parent_translation: Option<Vec3>,

    pub child_changes: HashMap<String, ChildChange>,

    pub commands: Vec<ScriptCommand>,

    // Environment outputs
    pub env_ambient_brightness: Option<f32>,
    pub env_ambient_color: Option<(f32, f32, f32)>,
    pub env_ev100: Option<f32>,
    pub env_sky_top_color: Option<(f32, f32, f32)>,
    pub env_sky_horizon_color: Option<(f32, f32, f32)>,
    pub env_sun_azimuth: Option<f32>,
    pub env_sun_elevation: Option<f32>,
    pub env_fog_enabled: Option<bool>,
    pub env_fog_color: Option<(f32, f32, f32)>,
    pub env_fog_start: Option<f32>,
    pub env_fog_end: Option<f32>,
}

impl ScriptContext {
    pub fn new(time: ScriptTime, transform: ScriptTransform) -> Self {
        Self {
            time,
            transform,
            input_movement: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            camera_yaw: 0.0,
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
            entities_by_tag: HashMap::new(),
            collisions_entered: Vec::new(),
            collisions_exited: Vec::new(),
            active_collisions: Vec::new(),
            timers_just_finished: Vec::new(),
            raycast_results: HashMap::new(),
            self_health: 0.0,
            self_max_health: 0.0,
            self_health_percent: 0.0,
            self_is_invincible: false,
            self_light_intensity: 0.0,
            self_light_color: [1.0, 1.0, 1.0],
            self_material_color: [1.0, 1.0, 1.0, 1.0],
            new_position: None,
            new_rotation: None,
            translation: None,
            rotation_delta: None,
            new_scale: None,
            look_at_target: None,
            parent_new_position: None,
            parent_new_rotation: None,
            parent_translation: None,
            child_changes: HashMap::new(),
            commands: Vec::new(),
            env_ambient_brightness: None,
            env_ambient_color: None,
            env_ev100: None,
            env_sky_top_color: None,
            env_sky_horizon_color: None,
            env_sun_azimuth: None,
            env_sun_elevation: None,
            env_fog_enabled: None,
            env_fog_color: None,
            env_fog_start: None,
            env_fog_end: None,
        }
    }

    /// Process a command, routing transform/environment commands to context fields
    /// and everything else to the commands vec.
    pub fn process_command(&mut self, cmd: ScriptCommand) {
        match cmd {
            ScriptCommand::SetPosition { x, y, z } => self.new_position = Some(Vec3::new(x, y, z)),
            ScriptCommand::SetRotation { x, y, z } => self.new_rotation = Some(Vec3::new(x, y, z)),
            ScriptCommand::SetScale { x, y, z } => self.new_scale = Some(Vec3::new(x, y, z)),
            ScriptCommand::Translate { x, y, z } => self.translation = Some(Vec3::new(x, y, z)),
            ScriptCommand::Rotate { x, y, z } => self.rotation_delta = Some(Vec3::new(x, y, z)),
            ScriptCommand::LookAt { x, y, z } => self.look_at_target = Some(Vec3::new(x, y, z)),
            ScriptCommand::ParentSetPosition { x, y, z } => self.parent_new_position = Some(Vec3::new(x, y, z)),
            ScriptCommand::ParentSetRotation { x, y, z } => self.parent_new_rotation = Some(Vec3::new(x, y, z)),
            ScriptCommand::ParentTranslate { x, y, z } => self.parent_translation = Some(Vec3::new(x, y, z)),
            ScriptCommand::ChildSetPosition { name, x, y, z } => {
                self.child_changes.entry(name).or_default().new_position = Some(Vec3::new(x, y, z));
            }
            ScriptCommand::ChildSetRotation { name, x, y, z } => {
                self.child_changes.entry(name).or_default().new_rotation = Some(Vec3::new(x, y, z));
            }
            ScriptCommand::ChildTranslate { name, x, y, z } => {
                self.child_changes.entry(name).or_default().translation = Some(Vec3::new(x, y, z));
            }
            ScriptCommand::SetSunAngles { azimuth, elevation } => {
                self.env_sun_azimuth = Some(azimuth);
                self.env_sun_elevation = Some(elevation);
            }
            ScriptCommand::SetAmbientBrightness { brightness } => self.env_ambient_brightness = Some(brightness),
            ScriptCommand::SetAmbientColor { r, g, b } => self.env_ambient_color = Some((r, g, b)),
            ScriptCommand::SetSkyTopColor { r, g, b } => self.env_sky_top_color = Some((r, g, b)),
            ScriptCommand::SetSkyHorizonColor { r, g, b } => self.env_sky_horizon_color = Some((r, g, b)),
            ScriptCommand::SetFog { enabled, start, end } => {
                self.env_fog_enabled = Some(enabled);
                self.env_fog_start = Some(start);
                self.env_fog_end = Some(end);
            }
            ScriptCommand::SetFogColor { r, g, b } => self.env_fog_color = Some((r, g, b)),
            ScriptCommand::SetEv100 { value } => self.env_ev100 = Some(value),
            other => self.commands.push(other),
        }
    }
}
