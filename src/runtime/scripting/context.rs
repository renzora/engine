//! Script execution context for the runtime

use bevy::prelude::*;
use std::collections::HashMap;

use super::commands::RhaiCommand;
use super::resources::RaycastHit;

/// Time information available to scripts
#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptTime {
    pub elapsed: f64,
    pub delta: f32,
    pub fixed_delta: f32,
    pub frame_count: u64,
}

/// Transform information available to scripts
#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptTransform {
    pub position: Vec3,
    pub rotation_euler: Vec3,
    pub scale: Vec3,
}

impl ScriptTransform {
    pub fn from_transform(transform: &Transform) -> Self {
        let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
        Self {
            position: transform.translation,
            rotation_euler: Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()),
            scale: transform.scale,
        }
    }
}

/// Context for Rhai script execution
#[derive(Clone, Debug)]
pub struct RhaiScriptContext {
    // Time
    pub time: ScriptTime,

    // Transform
    pub transform: ScriptTransform,

    // Entity info
    pub self_entity_id: u64,
    pub self_entity_name: String,

    // Entity lookups
    pub found_entities: HashMap<String, u64>,
    pub entities_by_tag: HashMap<String, Vec<u64>>,

    // Collision data
    pub collisions_entered: Vec<u64>,
    pub collisions_exited: Vec<u64>,
    pub active_collisions: Vec<u64>,

    // Timer data
    pub timers_just_finished: Vec<String>,

    // Raycast results
    pub raycast_results: HashMap<String, RaycastHit>,

    // Input
    pub input_movement: Vec2,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub gamepad_left_stick: Vec2,
    pub gamepad_right_stick: Vec2,

    // Output - commands and transform changes
    pub commands: Vec<RhaiCommand>,
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Quat>,
    pub new_scale: Option<Vec3>,
}

impl RhaiScriptContext {
    pub fn new(time: ScriptTime, transform: ScriptTransform) -> Self {
        Self {
            time,
            transform,
            self_entity_id: 0,
            self_entity_name: String::new(),
            found_entities: HashMap::new(),
            entities_by_tag: HashMap::new(),
            collisions_entered: Vec::new(),
            collisions_exited: Vec::new(),
            active_collisions: Vec::new(),
            timers_just_finished: Vec::new(),
            raycast_results: HashMap::new(),
            input_movement: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            gamepad_left_stick: Vec2::ZERO,
            gamepad_right_stick: Vec2::ZERO,
            commands: Vec::new(),
            new_position: None,
            new_rotation: None,
            new_scale: None,
        }
    }
}
