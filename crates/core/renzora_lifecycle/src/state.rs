//! Lifecycle runtime state — tracks execution state across frames.

use bevy::prelude::*;
use std::collections::HashMap;

use renzora_blueprint::graph::{NodeId, PinValue};

/// A simple countdown timer (avoids Bevy Timer API changes).
#[derive(Clone, Debug)]
pub struct LifecycleTimer {
    pub remaining: f32,
    pub duration: f32,
    pub repeat: bool,
}

impl LifecycleTimer {
    pub fn new(seconds: f32, repeat: bool) -> Self {
        Self {
            remaining: seconds,
            duration: seconds,
            repeat,
        }
    }

    /// Tick by delta. Returns true if the timer just completed.
    pub fn tick(&mut self, delta: f32) -> bool {
        self.remaining -= delta;
        if self.remaining <= 0.0 {
            if self.repeat {
                self.remaining += self.duration;
            }
            true
        } else {
            false
        }
    }
}

/// Runtime state for the lifecycle graph interpreter.
#[derive(Resource, Default)]
pub struct LifecycleRuntimeState {
    /// True after the first frame — prevents On Game Start from re-firing.
    pub initialized: bool,
    /// DoOnce: set of node IDs that have already fired.
    pub do_once_fired: HashMap<NodeId, bool>,
    /// FlipFlop: current side per node.
    pub flip_flop_state: HashMap<NodeId, bool>,
    /// Gate: open/closed per node.
    pub gate_open: HashMap<NodeId, bool>,
    /// User-defined lifecycle variables.
    pub variables: HashMap<String, PinValue>,
    /// Counter node accumulated values.
    pub counter_values: HashMap<NodeId, f32>,
    /// Active Wait timers: node_id → remaining seconds.
    pub active_waits: HashMap<NodeId, LifecycleTimer>,
    /// Continuations to resume when a wait timer completes: (node_id, pin_name).
    pub pending_continuations: Vec<(NodeId, String)>,
    /// Currently loaded scene name.
    pub current_scene: String,
    /// Set when a scene finishes loading — consumed by On Scene Loaded event.
    pub scene_just_loaded: Option<String>,
    /// Previous frame's connection state — for edge detection.
    pub prev_connected: bool,
    /// Named timers: name → timer.
    pub named_timers: HashMap<String, LifecycleTimer>,
    /// Timers that just finished this frame (name list).
    pub timers_just_finished: Vec<String>,
}
