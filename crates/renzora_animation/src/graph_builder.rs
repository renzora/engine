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
pub fn resolve_blend_tree_weights(tree: &BlendTree, params: &AnimParams) -> Vec<(String, f32)> {
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
        BlendTree::BlendSpace2D {
            entries,
            param_x,
            param_y,
        } => {
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
        BlendTree::Additive {
            base,
            overlay,
            param,
        } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_tree::BlendSpaceEntry;

    fn clip(name: &str) -> Box<BlendTree> {
        Box::new(BlendTree::Clip(name.to_string()))
    }

    fn params(pairs: &[(&str, f32)]) -> AnimParams {
        let mut p = AnimParams::default();
        for (k, v) in pairs {
            p.set_float(*k, *v);
        }
        p
    }

    fn weight_of(weights: &[(String, f32)], clip: &str) -> f32 {
        weights
            .iter()
            .filter(|(n, _)| n == clip)
            .map(|(_, w)| *w)
            .sum()
    }

    fn total(weights: &[(String, f32)]) -> f32 {
        weights.iter().map(|(_, w)| *w).sum()
    }

    #[test]
    fn a_lone_clip_plays_at_full_weight() {
        let w = resolve_blend_tree_weights(&BlendTree::Clip("idle".into()), &AnimParams::default());
        assert_eq!(w, vec![("idle".to_string(), 1.0)]);
    }

    /// A lerp must always be a *partition* of 1.0. If the two sides ever sum to
    /// less, the character visibly sinks toward its rest pose mid-blend; more,
    /// and the pose over-shoots.
    #[test]
    fn a_lerp_always_partitions_one_unit_of_weight() {
        let tree = BlendTree::Lerp { a: clip("walk"), b: clip("run"), param: "speed".into() };
        for t in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let w = resolve_blend_tree_weights(&tree, &params(&[("speed", t)]));
            assert!((total(&w) - 1.0).abs() < 1e-5, "t={t} summed to {}", total(&w));
            assert!((weight_of(&w, "walk") - (1.0 - t)).abs() < 1e-5, "t={t}");
            assert!((weight_of(&w, "run") - t).abs() < 1e-5, "t={t}");
        }
    }

    /// The parameter is authored by gameplay code, which has no obligation to
    /// stay in 0..1 — a speed parameter fed raw velocity routinely will not.
    /// Unclamped, `t = 3.0` gives the B clip weight 3 and the A clip weight −2.
    #[test]
    fn a_lerp_clamps_an_out_of_range_parameter() {
        let tree = BlendTree::Lerp { a: clip("walk"), b: clip("run"), param: "speed".into() };

        let over = resolve_blend_tree_weights(&tree, &params(&[("speed", 5.0)]));
        assert_eq!(weight_of(&over, "run"), 1.0);
        assert_eq!(weight_of(&over, "walk"), 0.0);

        let under = resolve_blend_tree_weights(&tree, &params(&[("speed", -5.0)]));
        assert_eq!(weight_of(&under, "walk"), 1.0);
        assert_eq!(weight_of(&under, "run"), 0.0);
    }

    /// A parameter nobody has set reads as 0.0, which must mean "fully A" rather
    /// than a panic or an empty result — a state machine can enter a blend tree
    /// before gameplay has written its parameter even once.
    #[test]
    fn a_missing_parameter_resolves_to_the_a_side() {
        let tree = BlendTree::Lerp { a: clip("walk"), b: clip("run"), param: "never_set".into() };
        let w = resolve_blend_tree_weights(&tree, &AnimParams::default());
        assert_eq!(weight_of(&w, "walk"), 1.0);
        assert_eq!(weight_of(&w, "run"), 0.0);
    }

    /// Nested lerps must still partition 1.0 overall — the inner tree's weights
    /// are scaled by the outer blend, so an error compounds rather than cancels.
    #[test]
    fn nested_lerps_still_partition_one_unit() {
        let tree = BlendTree::Lerp {
            a: Box::new(BlendTree::Lerp {
                a: clip("idle"),
                b: clip("walk"),
                param: "low".into(),
            }),
            b: clip("run"),
            param: "high".into(),
        };
        for (low, high) in [(0.0f32, 0.0f32), (0.5, 0.5), (1.0, 0.25), (0.3, 0.8)] {
            let w = resolve_blend_tree_weights(&tree, &params(&[("low", low), ("high", high)]));
            assert!(
                (total(&w) - 1.0).abs() < 1e-5,
                "low={low} high={high} summed to {}",
                total(&w)
            );
        }
    }

    // ── 2D blend space ───────────────────────────────────────────────────────

    fn locomotion_space() -> BlendTree {
        BlendTree::BlendSpace2D {
            entries: vec![
                BlendSpaceEntry { clip: "idle".into(), x: 0.0, y: 0.0 },
                BlendSpaceEntry { clip: "fwd".into(), x: 0.0, y: 1.0 },
                BlendSpaceEntry { clip: "back".into(), x: 0.0, y: -1.0 },
                BlendSpaceEntry { clip: "right".into(), x: 1.0, y: 0.0 },
            ],
            param_x: "x".into(),
            param_y: "y".into(),
        }
    }

    #[test]
    fn a_blend_space_normalizes_to_one_unit_of_weight() {
        let tree = locomotion_space();
        for (x, y) in [(0.0f32, 0.0f32), (0.5, 0.5), (-2.0, 3.0), (0.1, -0.9)] {
            let w = resolve_blend_tree_weights(&tree, &params(&[("x", x), ("y", y)]));
            assert!(
                (total(&w) - 1.0).abs() < 1e-4,
                "({x},{y}) summed to {}",
                total(&w)
            );
        }
    }

    /// Standing exactly on an entry must play essentially that clip alone. The
    /// distance floor of 0.001 is what makes this work — a true zero distance
    /// would divide by zero and produce NaN weights, which propagate into the
    /// pose and freeze the character.
    #[test]
    fn standing_on_an_entry_plays_almost_only_that_clip() {
        let w = resolve_blend_tree_weights(&locomotion_space(), &params(&[("x", 0.0), ("y", 1.0)]));
        assert!(
            weight_of(&w, "fwd") > 0.99,
            "expected fwd to dominate, got {}",
            weight_of(&w, "fwd")
        );
        assert!(w.iter().all(|(_, x)| x.is_finite()), "produced a non-finite weight");
    }

    /// Halfway between two entries, both should carry real weight — that is the
    /// entire point of a blend space.
    #[test]
    fn a_point_between_entries_blends_them() {
        let w = resolve_blend_tree_weights(&locomotion_space(), &params(&[("x", 0.0), ("y", 0.5)]));
        assert!(weight_of(&w, "idle") > 0.05);
        assert!(weight_of(&w, "fwd") > 0.05);
        assert!(weight_of(&w, "idle") > weight_of(&w, "back"), "the nearer clip should win");
    }

    #[test]
    fn a_nearer_entry_always_outweighs_a_further_one() {
        let w = resolve_blend_tree_weights(&locomotion_space(), &params(&[("x", 0.0), ("y", 0.8)]));
        assert!(weight_of(&w, "fwd") > weight_of(&w, "idle"));
        assert!(weight_of(&w, "idle") > weight_of(&w, "back"));
    }

    #[test]
    fn an_empty_blend_space_resolves_to_no_clips() {
        let tree = BlendTree::BlendSpace2D {
            entries: vec![],
            param_x: "x".into(),
            param_y: "y".into(),
        };
        assert!(resolve_blend_tree_weights(&tree, &AnimParams::default()).is_empty());
    }

    #[test]
    fn a_one_entry_blend_space_plays_that_clip_at_full_weight() {
        let tree = BlendTree::BlendSpace2D {
            entries: vec![BlendSpaceEntry { clip: "only".into(), x: 5.0, y: 5.0 }],
            param_x: "x".into(),
            param_y: "y".into(),
        };
        let w = resolve_blend_tree_weights(&tree, &params(&[("x", -100.0), ("y", 100.0)]));
        assert_eq!(w.len(), 1);
        assert!((w[0].1 - 1.0).abs() < 1e-5, "a lone entry must normalize to 1.0");
    }

    // ── additive ─────────────────────────────────────────────────────────────

    /// Additive is *not* a partition: the base keeps its full weight and the
    /// overlay is layered on top. Scaling the base down would make a character
    /// sink toward rest whenever it waved.
    #[test]
    fn an_additive_overlay_does_not_dilute_its_base() {
        let tree = BlendTree::Additive {
            base: clip("run"),
            overlay: clip("wave"),
            param: "wave".into(),
        };
        for amount in [0.0f32, 0.5, 1.0] {
            let w = resolve_blend_tree_weights(&tree, &params(&[("wave", amount)]));
            assert_eq!(weight_of(&w, "run"), 1.0, "base was scaled at {amount}");
            assert!((weight_of(&w, "wave") - amount).abs() < 1e-5);
        }
    }

    #[test]
    fn an_additive_clamps_its_parameter() {
        let tree = BlendTree::Additive {
            base: clip("run"),
            overlay: clip("wave"),
            param: "wave".into(),
        };
        let over = resolve_blend_tree_weights(&tree, &params(&[("wave", 9.0)]));
        assert_eq!(weight_of(&over, "wave"), 1.0);
        let under = resolve_blend_tree_weights(&tree, &params(&[("wave", -9.0)]));
        assert_eq!(weight_of(&under, "wave"), 0.0);
    }

    /// Every shape must produce finite weights for any parameter value —
    /// a NaN reaching `set_weight` freezes the character with no error.
    #[test]
    fn no_tree_shape_ever_produces_a_non_finite_weight() {
        let trees = vec![
            BlendTree::Clip("a".into()),
            BlendTree::Lerp { a: clip("a"), b: clip("b"), param: "p".into() },
            locomotion_space(),
            BlendTree::Additive { base: clip("a"), overlay: clip("b"), param: "p".into() },
        ];
        for tree in &trees {
            for v in [0.0f32, 1.0, -1.0, 1e9, -1e9] {
                let w = resolve_blend_tree_weights(
                    tree,
                    &params(&[("p", v), ("x", v), ("y", v)]),
                );
                assert!(
                    w.iter().all(|(_, x)| x.is_finite()),
                    "{tree:?} at {v} produced a non-finite weight: {w:?}"
                );
            }
        }
    }
}
