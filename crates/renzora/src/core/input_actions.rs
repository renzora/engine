//! The input → physics/scripting contract: named actions, and the character
//! controller queue they usually drive.
//!
//! `renzora_input` writes [`ActionState`] each frame from the keybinding map;
//! physics, scripting and blueprints all read it. Nobody links anybody, because
//! an action is a string and its state is plain data.

/// Queued character controller commands, processed by renzora_physics each frame.
#[derive(bevy::prelude::Resource, Default)]
pub struct CharacterCommandQueue {
    pub commands: Vec<(bevy::ecs::entity::Entity, CharacterCommand)>,
}

/// A character controller command for a specific entity.
#[derive(Debug)]
pub enum CharacterCommand {
    Move(bevy::prelude::Vec2),
    Jump,
    Sprint(bool),
}

/// Per-action runtime state computed each frame by the input system.
#[derive(Clone, Debug, Default)]
pub struct ActionData {
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
    pub axis_1d: f32,
    pub axis_2d: bevy::prelude::Vec2,
}

/// Computed action states, populated by the input system and read by
/// physics, scripting, and blueprints each frame.
#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct ActionState {
    pub actions: std::collections::HashMap<String, ActionData>,
}

impl ActionState {
    pub fn pressed(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.pressed)
    }
    pub fn just_pressed(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.just_pressed)
    }
    pub fn just_released(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.just_released)
    }
    pub fn axis_1d(&self, action: &str) -> f32 {
        self.actions.get(action).map_or(0.0, |a| a.axis_1d)
    }
    pub fn axis_2d(&self, action: &str) -> bevy::prelude::Vec2 {
        self.actions
            .get(action)
            .map_or(bevy::prelude::Vec2::ZERO, |a| a.axis_2d)
    }
}
