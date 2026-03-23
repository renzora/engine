//! Graph builder — translates state machine + blend trees into Bevy AnimationGraph.
//!
//! The state machine and blend trees are high-level abstractions. At runtime,
//! we flatten them into a Bevy AnimationGraph with blend nodes and transitions.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::blend_tree::BlendTree;
use crate::component::AnimatorState;
use crate::state_machine::{AnimParams, AnimationStateMachine, StateMotion};

/// Build/rebuild the Bevy AnimationGraph from the current state machine state.
///
/// For Phase 2 we use a simple approach: each clip gets a node in the graph,
/// and we drive transitions via AnimationTransitions. Blend trees set weights
/// on the active nodes.
pub fn build_graph_from_state_machine(
    sm: &AnimationStateMachine,
    clip_handles: &HashMap<String, Handle<AnimationClip>>,
) -> (AnimationGraph, HashMap<String, AnimationNodeIndex>) {
    // Collect all unique clips referenced by the state machine
    let mut clip_names: Vec<String> = Vec::new();
    for state in &sm.states {
        match &state.motion {
            StateMotion::Clip(name) => {
                if !clip_names.contains(name) {
                    clip_names.push(name.clone());
                }
            }
            StateMotion::BlendTree(_) => {
                // Blend tree clips will be collected when we have the tree definitions
                // For now, all clips in clip_handles are available
            }
        }
    }

    // Ensure all clip_handles keys are included
    for name in clip_handles.keys() {
        if !clip_names.contains(name) {
            clip_names.push(name.clone());
        }
    }

    // Build graph from the available handles
    let mut handles_ordered: Vec<(String, Handle<AnimationClip>)> = Vec::new();
    for name in &clip_names {
        if let Some(handle) = clip_handles.get(name) {
            handles_ordered.push((name.clone(), handle.clone()));
        }
    }

    let (graph, node_indices) =
        AnimationGraph::from_clips(handles_ordered.iter().map(|(_, h)| h.clone()));

    let mut name_to_index = HashMap::new();
    for (i, (name, _)) in handles_ordered.iter().enumerate() {
        name_to_index.insert(name.clone(), node_indices[i]);
    }

    (graph, name_to_index)
}

/// Resolve which clip(s) a blend tree requires and their weights.
/// Returns a list of (clip_name, weight) pairs.
pub fn resolve_blend_tree_weights(
    tree: &BlendTree,
    params: &AnimParams,
) -> Vec<(String, f32)> {
    match tree {
        BlendTree::Clip(name) => vec![(name.clone(), 1.0)],
        BlendTree::Lerp { a, b, param } => {
            let t = params.get_float(param).clamp(0.0, 1.0);
            let mut result = Vec::new();
            for (name, w) in resolve_blend_tree_weights(a, params) {
                result.push((name, w * (1.0 - t)));
            }
            for (name, w) in resolve_blend_tree_weights(b, params) {
                result.push((name, w * t));
            }
            result
        }
        BlendTree::BlendSpace2D { entries, param_x, param_y } => {
            if entries.is_empty() {
                return Vec::new();
            }

            let px = params.get_float(param_x);
            let py = params.get_float(param_y);

            // Inverse-distance weighting
            let mut weights: Vec<(String, f32)> = Vec::new();
            let mut total_weight = 0.0f32;

            for entry in entries {
                let dx = px - entry.x;
                let dy = py - entry.y;
                let dist = (dx * dx + dy * dy).sqrt().max(0.001);
                let w = 1.0 / dist;
                weights.push((entry.clip.clone(), w));
                total_weight += w;
            }

            // Normalize
            if total_weight > 0.0 {
                for (_, w) in &mut weights {
                    *w /= total_weight;
                }
            }

            weights
        }
        BlendTree::Additive { base, overlay, param } => {
            let overlay_weight = params.get_float(param).clamp(0.0, 1.0);
            let mut result = Vec::new();
            for (name, w) in resolve_blend_tree_weights(base, params) {
                result.push((name, w));
            }
            for (name, w) in resolve_blend_tree_weights(overlay, params) {
                result.push((name, w * overlay_weight));
            }
            result
        }
    }
}

/// Apply blend tree weights to the animation player by adjusting individual
/// animation weights on the active nodes.
pub fn apply_blend_weights(
    player: &mut AnimationPlayer,
    state: &AnimatorState,
    weights: &[(String, f32)],
) {
    // First, zero out all node weights
    for &node_idx in state.node_indices.values() {
        if let Some(anim) = player.animation_mut(node_idx) {
            anim.set_weight(0.0);
        }
    }

    // Apply the blend tree weights
    for (name, weight) in weights {
        if let Some(&node_idx) = state.node_indices.get(name) {
            if let Some(anim) = player.animation_mut(node_idx) {
                anim.set_weight(*weight);
            }
        }
    }
}
