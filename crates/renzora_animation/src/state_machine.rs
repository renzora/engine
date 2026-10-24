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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateMotion {
    /// Play a single clip by slot name.
    Clip(String),
    /// A named blend tree reference, from before trees had anywhere to be
    /// stored. Nothing ever wrote a tree under the name, so a file holding one
    /// plays the slot of that name, which is what such a state was pointing at.
    BlendTree(String),
    /// A blend tree stored inline in the state, driven by the machine's
    /// parameters every frame.
    ///
    /// Inline rather than in a table on the machine so that adding trees took
    /// no new field on [`AnimationStateMachine`] or [`AnimState`], which every
    /// existing `.animsm` file (and every tool that builds one) would then have
    /// had to spell.
    Blend(crate::blend_tree::BlendTree),
}

impl StateMotion {
    /// Every clip this motion can play.
    pub fn clips(&self) -> Vec<String> {
        match self {
            StateMotion::Clip(name) | StateMotion::BlendTree(name) => vec![name.clone()],
            StateMotion::Blend(tree) => {
                let mut out = Vec::new();
                tree.collect_clips(&mut out);
                out
            }
        }
    }
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
            AnimCondition::FloatGreater(name, threshold) => params.get_float(name) > *threshold,
            AnimCondition::FloatLess(name, threshold) => params.get_float(name) < *threshold,
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
        self.transitions
            .iter()
            .find(|t| t.from == current_state && t.condition.evaluate(params, state_time))
    }

    /// Clips any state's blend tree plays additively. The graph builder needs
    /// them up front: an additive node and a difference clip exist only for
    /// the clips something asks to add.
    pub fn additive_clips(&self) -> Vec<String> {
        let mut out = Vec::new();
        for state in &self.states {
            if let StateMotion::Blend(tree) = &state.motion {
                tree.collect_additive_clips(&mut out);
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_tree::{Blend1DEntry, BlendTree};

    /// A `.animsm` written before inline trees existed must still load, and
    /// one with a tree must round-trip.
    #[test]
    fn old_and_new_state_machines_load() {
        let old = r#"(states:[(name:"idle",motion:Clip("idle"),speed:1.0,looping:true)],transitions:[],default_state:"idle")"#;
        let sm: AnimationStateMachine = ron::from_str(old).unwrap();
        assert_eq!(sm.states[0].motion, StateMotion::Clip("idle".into()));

        let mut with_tree = sm.clone();
        with_tree.states.push(AnimState {
            name: "move".into(),
            motion: StateMotion::Blend(BlendTree::Blend1D {
                param: "speed".into(),
                entries: vec![
                    Blend1DEntry { clip: "walk".into(), threshold: 1.0 },
                    Blend1DEntry { clip: "run".into(), threshold: 4.0 },
                ],
            }),
            speed: 1.0,
            looping: true,
        });
        let text = ron::ser::to_string(&with_tree).unwrap();
        let back: AnimationStateMachine = ron::from_str(&text).unwrap();
        assert_eq!(back.states[1].motion, with_tree.states[1].motion);
        assert_eq!(back.states[1].motion.clips(), vec!["walk", "run"]);
    }
}
