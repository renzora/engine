//! AnimatorComponent — drives skeletal animation from .anim clip slots.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One named animation clip slot.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct AnimClipSlot {
    /// Label used by scripting ("run", "idle", etc.)
    pub name: String,
    /// Asset path to the .anim file (relative to project).
    pub path: String,
    pub looping: bool,
    pub speed: f32,
    /// Runtime: loaded handle.
    #[reflect(ignore)]
    #[serde(skip)]
    pub handle: Option<Handle<AnimationClip>>,
    /// Runtime: node index in the animation graph.
    #[reflect(ignore)]
    #[serde(skip)]
    pub node_index: Option<AnimationNodeIndex>,
}

impl AnimClipSlot {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            looping: true,
            speed: 1.0,
            handle: None,
            node_index: None,
        }
    }
}

/// Component that manages animation clips for a character entity.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct AnimatorComponent {
    pub clips: Vec<AnimClipSlot>,
    /// Name of the currently active clip slot (matches `AnimClipSlot::name`).
    pub current_clip: Option<String>,
    /// Crossfade duration in seconds.
    pub blend_duration: f32,
    /// Runtime: whether the graph has been built.
    #[reflect(ignore)]
    #[serde(skip)]
    pub initialized: bool,
    /// Runtime: which entity has the AnimationPlayer (found in children).
    #[reflect(ignore)]
    #[serde(skip)]
    pub player_entity: Option<Entity>,
}

impl AnimatorComponent {
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            current_clip: None,
            blend_duration: 0.2,
            initialized: false,
            player_entity: None,
        }
    }

    pub fn with_blend_duration(mut self, secs: f32) -> Self {
        self.blend_duration = secs;
        self
    }

    pub fn add_clip(&mut self, slot: AnimClipSlot) {
        self.clips.push(slot);
    }

    /// Find a slot by name.
    pub fn get_slot(&self, name: &str) -> Option<&AnimClipSlot> {
        self.clips.iter().find(|s| s.name == name)
    }

    pub fn get_slot_mut(&mut self, name: &str) -> Option<&mut AnimClipSlot> {
        self.clips.iter_mut().find(|s| s.name == name)
    }
}
