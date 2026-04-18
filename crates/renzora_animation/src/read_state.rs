//! Per-entity script-visible animator mirror.
//!
//! Populated each frame from `AnimatorState` + the loaded `AnimationClip`
//! assets. Scripts and blueprints read this via reflection path dispatch
//! (`get("AnimatorReadState.current_clip")` etc.) without depending on Bevy
//! or Avian types.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::component::{AnimatorComponent, AnimatorState};

/// Read-only snapshot of animator state. Never saved to scenes.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AnimatorReadState {
    /// Name of the currently-playing clip slot, or empty if none.
    pub current_clip: String,
    /// Name of the current state machine state, or empty if none.
    pub current_state: String,
    /// Seconds spent in the current state.
    pub state_time: f32,
    /// Per-clip duration in seconds. Populated once clips load.
    pub clip_lengths: HashMap<String, f32>,
    /// Current float parameters on the state machine.
    pub params: HashMap<String, f32>,
    /// Current bool parameters on the state machine.
    pub bool_params: HashMap<String, bool>,
}

/// Auto-inserts [`AnimatorReadState`] whenever an entity has an
/// `AnimatorComponent` but no mirror yet.
pub fn auto_init_animator_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<AnimatorComponent>, Without<AnimatorReadState>)>,
) {
    for entity in &q {
        commands.entity(entity).try_insert(AnimatorReadState::default());
    }
}

/// Refreshes [`AnimatorReadState`] from `AnimatorState` + `Assets<AnimationClip>`.
pub fn update_animator_read_state(
    mut q: Query<(&AnimatorState, &mut AnimatorReadState)>,
    clips: Res<Assets<AnimationClip>>,
) {
    for (state, mut rs) in &mut q {
        rs.current_clip = state.current_clip.clone().unwrap_or_default();
        rs.current_state = state.current_state.clone().unwrap_or_default();
        rs.state_time = state.state_time;
        for (name, handle) in &state.clip_handles {
            if let Some(clip) = clips.get(handle) {
                rs.clip_lengths.insert(name.clone(), clip.duration());
            }
        }
        rs.params = state.params.floats.clone();
        rs.bool_params = state.params.bools.clone();
    }
}
