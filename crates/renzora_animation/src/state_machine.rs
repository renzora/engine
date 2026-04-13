//! Animation State Machine — states, transitions, conditions, and parameters.
//!
//! Serialized as `.animsm` files (RON format). At runtime, the state machine
//! evaluates conditions each frame and drives clip/blend-tree selection.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete animation state machine definition.
/// Saved as a `.animsm` file (RON format).
#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct AnimationStateMachine {
    /// Named states in this machine.
    pub states: Vec<AnimState>,
    /// Transitions between states.
    pub transitions: Vec<AnimTransition>,
    /// Which state to enter on startup.
    pub default_state: String,
}

/// A single state in the state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimState {
    /// Unique name for this state (e.g. "idle", "walk", "run").
    pub name: String,
    /// Which clip or blend tree to play in this state.
    pub motion: StateMotion,
    /// Playback speed multiplier.
    pub speed: f32,
    /// Whether the motion loops.
    pub looping: bool,
}

/// What drives playback in a state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateMotion {
    /// Play a single clip by slot name.
    Clip(String),
    /// Use a blend tree (inline or by name reference).
    BlendTree(String),
}

/// A transition between two states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimTransition {
    /// Source state name.
    pub from: String,
    /// Destination state name.
    pub to: String,
    /// Condition that triggers this transition.
    pub condition: AnimCondition,
    /// Crossfade duration in seconds.
    pub blend_duration: f32,
}

/// Condition for triggering a state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimCondition {
    /// Float parameter > threshold.
    FloatGreater(String, f32),
    /// Float parameter < threshold.
    FloatLess(String, f32),
    /// Bool parameter is true.
    BoolTrue(String),
    /// Bool parameter is false.
    BoolFalse(String),
    /// One-shot trigger was fired.
    Trigger(String),
    /// Current state has played for at least this many seconds.
    TimeElapsed(f32),
    /// Always true (immediate transition).
    Always,
}

/// Runtime parameter values for a state machine.
#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct AnimParams {
    pub floats: HashMap<String, f32>,
    pub bools: HashMap<String, bool>,
    pub triggers: HashMap<String, bool>,
}

impl AnimParams {
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) {
        self.floats.insert(name.into(), value);
    }

    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.bools.insert(name.into(), value);
    }

    pub fn fire_trigger(&mut self, name: impl Into<String>) {
        self.triggers.insert(name.into(), true);
    }

    pub fn consume_trigger(&mut self, name: &str) -> bool {
        self.triggers.remove(name).unwrap_or(false)
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.floats.get(name).copied().unwrap_or(0.0)
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.bools.get(name).copied().unwrap_or(false)
    }
}

impl AnimCondition {
    /// Evaluate this condition against the current parameters and elapsed time.
    pub fn evaluate(&self, params: &AnimParams, state_time: f32) -> bool {
        match self {
            AnimCondition::FloatGreater(name, threshold) => {
                params.get_float(name) > *threshold
            }
            AnimCondition::FloatLess(name, threshold) => {
                params.get_float(name) < *threshold
            }
            AnimCondition::BoolTrue(name) => params.get_bool(name),
            AnimCondition::BoolFalse(name) => !params.get_bool(name),
            AnimCondition::Trigger(name) => {
                // Triggers are consumed in the state machine update, not here.
                // Check returns true if the trigger is set.
                params.triggers.get(name).copied().unwrap_or(false)
            }
            AnimCondition::TimeElapsed(duration) => state_time >= *duration,
            AnimCondition::Always => true,
        }
    }
}

impl AnimationStateMachine {
    pub fn get_state(&self, name: &str) -> Option<&AnimState> {
        self.states.iter().find(|s| s.name == name)
    }

    /// Find the first valid transition from the given state.
    pub fn evaluate_transitions(
        &self,
        current_state: &str,
        params: &AnimParams,
        state_time: f32,
    ) -> Option<&AnimTransition> {
        self.transitions.iter().find(|t| {
            t.from == current_state && t.condition.evaluate(params, state_time)
        })
    }
}
