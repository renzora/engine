//! AnimatorComponent — scene-serializable animation controller.
//! AnimatorState — ephemeral runtime state rebuilt on load.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::layers::AnimationLayer;
use crate::state_machine::{AnimParams, AnimationStateMachine};

/// Serializable clip slot: a named reference to a `.anim` file.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct AnimClipSlot {
    /// Label used by scripting / blueprints ("idle", "walk", etc.).
    pub name: String,
    /// Asset-relative path to the `.anim` file (e.g. `animations/walk.anim`).
    pub path: String,
    /// Whether this clip loops by default.
    pub looping: bool,
    /// Default playback speed.
    pub speed: f32,
}

impl AnimClipSlot {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            looping: true,
            speed: 1.0,
        }
    }
}

/// Scene-persistent component that defines which animation clips an entity has.
///
/// Attach this to a GLTF model entity. The runtime will build an AnimationGraph,
/// locate the AnimationPlayer in children, and drive playback.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AnimatorComponent {
    /// Named clip slots.
    pub clips: Vec<AnimClipSlot>,
    /// Which clip to auto-play on spawn (matches `AnimClipSlot::name`).
    pub default_clip: Option<String>,
    /// Crossfade duration in seconds when transitioning between clips.
    pub blend_duration: f32,
    /// Optional path to a `.animsm` state machine file.
    pub state_machine: Option<String>,
    /// Animation layers (base layer is index 0).
    pub layers: Vec<AnimationLayer>,
}

impl AnimatorComponent {
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            default_clip: None,
            blend_duration: 0.2,
            state_machine: None,
            layers: Vec::new(),
        }
    }

    pub fn add_clip(&mut self, slot: AnimClipSlot) {
        self.clips.push(slot);
    }

    pub fn get_slot(&self, name: &str) -> Option<&AnimClipSlot> {
        self.clips.iter().find(|s| s.name == name)
    }

    pub fn get_slot_mut(&mut self, name: &str) -> Option<&mut AnimClipSlot> {
        self.clips.iter_mut().find(|s| s.name == name)
    }
}

/// Runtime-only state for an animator. Not serialized — rebuilt on scene load.
#[derive(Component)]
pub struct AnimatorState {
    /// Currently active clip name.
    pub current_clip: Option<String>,
    /// Whether playback is paused.
    pub is_paused: bool,
    /// Loaded clip handles keyed by slot name.
    pub clip_handles: HashMap<String, Handle<AnimationClip>>,
    /// Node index in the AnimationGraph per clip name.
    pub node_indices: HashMap<String, AnimationNodeIndex>,
    /// The AnimationGraph handle assigned to the player.
    pub graph_handle: Option<Handle<AnimationGraph>>,
    /// The entity that has the AnimationPlayer (found in children).
    pub player_entity: Option<Entity>,
    /// Whether initialization is complete.
    pub initialized: bool,
    /// Loaded state machine asset handle.
    pub sm_handle: Option<Handle<AnimationStateMachine>>,
    /// Current state name in the state machine.
    pub current_state: Option<String>,
    /// Time spent in the current state (seconds).
    pub state_time: f32,
    /// Runtime animation parameters (floats, bools, triggers).
    pub params: AnimParams,
}

impl Default for AnimatorState {
    fn default() -> Self {
        Self {
            current_clip: None,
            is_paused: false,
            clip_handles: HashMap::new(),
            node_indices: HashMap::new(),
            graph_handle: None,
            player_entity: None,
            initialized: false,
            sm_handle: None,
            current_state: None,
            state_time: 0.0,
            params: AnimParams::default(),
        }
    }
}
