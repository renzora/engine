//! Graph builder: lays an animator out as a Bevy `AnimationGraph`, and
//! resolves blend tree weights.
//!
//! # The shape of the graph
//!
//! ```text
//! root (Blend)
//! └─ layer N wrapper (Blend for Override, Add for Additive)
//!    ├─ layer N-1 wrapper ... down to the base
//!    │  └─ base (Add, only when a blend tree adds an overlay)
//!    │     ├─ base blend (Blend): one clip node per slot   <- `GraphLayout::base`
//!    │     └─ overlay difference clips per additive slot     <- `GraphLayout::overlay`
//!    └─ layer N clip nodes per slot, masked                  <- `GraphLayout::layers[N]`
//! ```
//!
//! Every weight the runtime changes is a clip's *playback* weight
//! (`ActiveAnimation::weight`), never a graph node's. Changing a node weight
//! modifies the graph asset, and Bevy re-threads a modified graph for every
//! target, so fading a layer would have rebuilt the graph every frame.
//!
//! That is why an override layer's clips sit directly in its wrapper, beside
//! the layers below, instead of under a blend of their own: Bevy normalises a
//! blend node's children, so a layer at weight `w` is a lerp only when its
//! clips weigh `w / (1 - w)` against the lower stack's `1`. A blend node
//! between them would normalise that away, and pushes its own (fixed) node
//! weight instead.
//!
//! Masks are Bevy's own: layer `k` uses mask group `k`, every bone the layer
//! must not touch is put in that group, and the layer's clip nodes mask it.

use bevy::animation::AnimationTargetId;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::blend_tree::BlendTree;
use crate::layers::LayerBlendMode;
use crate::state_machine::AnimParams;

/// What a clip node in the graph is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    /// The slot in the base motion (clips, crossfades, blend trees).
    Base,
    /// The slot's difference clip, added on top of the base by a blend tree.
    Overlay,
    /// The slot on layer `k`.
    Layer(usize),
}

/// Where each slot's clip nodes ended up, by role.
#[derive(Clone, Debug, Default)]
pub struct GraphLayout {
    pub base: HashMap<String, AnimationNodeIndex>,
    pub overlay: HashMap<String, AnimationNodeIndex>,
    pub layers: Vec<LayerNodes>,
    /// Reverse lookup: what a node plays and for whom.
    pub roles: HashMap<AnimationNodeIndex, (String, NodeRole)>,
}

/// One layer's clip nodes.
#[derive(Clone, Debug, Default)]
pub struct LayerNodes {
    pub mode: LayerBlendMode,
    pub nodes: HashMap<String, AnimationNodeIndex>,
}

/// One layer, as the builder needs it.
#[derive(Clone, Debug, Default)]
pub struct LayerSpec {
    pub mode: LayerBlendMode,
    /// Targets this layer must leave alone. `None` means the layer is unmasked.
    pub masked_out: Option<Vec<AnimationTargetId>>,
}

/// Everything the graph is built from.
pub struct GraphSpec<'a> {
    /// The clip each slot plays, in slot order.
    pub base: &'a [(String, Handle<AnimationClip>)],
    /// Difference clips (see [`crate::retarget::make_additive`]) by slot.
    pub additive: &'a HashMap<String, Handle<AnimationClip>>,
    /// Slots a blend tree adds on top of the base.
    pub overlay_slots: &'a [String],
    pub layers: &'a [LayerSpec],
}

/// Bevy's mask is a `u64`, so only the first 64 layers can be masked.
const MAX_MASKED_LAYERS: usize = 64;

/// Build an animator's graph. See the module docs for the shape.
pub fn build_animator_graph(spec: &GraphSpec) -> (AnimationGraph, GraphLayout) {
    let mut graph = AnimationGraph::new();
    let mut layout = GraphLayout::default();
    let overlays: Vec<&String> = spec
        .overlay_slots
        .iter()
        .filter(|s| spec.additive.contains_key(*s))
        .collect();
    let base_is_add = !overlays.is_empty();

    let level_node = |graph: &mut AnimationGraph, level: usize, parent: AnimationNodeIndex| {
        let additive = if level == 0 {
            base_is_add
        } else {
            spec.layers[level - 1].mode == LayerBlendMode::Additive
        };
        if additive {
            graph.add_additive_blend(1.0, parent)
        } else {
            graph.add_blend(1.0, parent)
        }
    };

    let root = graph.root;
    let mut node = level_node(&mut graph, spec.layers.len(), root);
    for level in (1..=spec.layers.len()).rev() {
        // The lower stack first: in an Add node the first child evaluated is
        // the one the rest are added onto, and siblings evaluate in index order.
        let below = level_node(&mut graph, level - 1, node);
        let k = level - 1;
        let layer = &spec.layers[k];
        let mask = match &layer.masked_out {
            Some(targets) if k < MAX_MASKED_LAYERS => {
                for target in targets {
                    graph.add_target_to_mask_group(*target, k as u32);
                }
                1u64 << k
            }
            Some(_) => {
                warn!("[animation] layer {k} has a mask, but only the first {MAX_MASKED_LAYERS} layers can be masked");
                0
            }
            None => 0,
        };
        let mut nodes = HashMap::new();
        for (slot, handle) in spec.base {
            let clip = match layer.mode {
                LayerBlendMode::Override => Some(handle.clone()),
                LayerBlendMode::Additive => spec.additive.get(slot).cloned(),
            };
            if let Some(clip) = clip {
                let idx = graph.add_clip_with_mask(clip, mask, 1.0, node);
                nodes.insert(slot.clone(), idx);
                layout.roles.insert(idx, (slot.clone(), NodeRole::Layer(k)));
            }
        }
        layout.layers.push(LayerNodes { mode: layer.mode, nodes });
        node = below;
    }
    // Built top-down, so put the layers back in their authored order.
    layout.layers.reverse();

    let base_blend = if base_is_add { graph.add_blend(1.0, node) } else { node };
    for (slot, handle) in spec.base {
        let idx = graph.add_clip(handle.clone(), 1.0, base_blend);
        layout.base.insert(slot.clone(), idx);
        layout.roles.insert(idx, (slot.clone(), NodeRole::Base));
    }
    if base_is_add {
        for slot in overlays {
            if let Some(handle) = spec.additive.get(slot) {
                let idx = graph.add_clip(handle.clone(), 1.0, node);
                layout.overlay.insert(slot.clone(), idx);
                layout.roles.insert(idx, (slot.clone(), NodeRole::Overlay));
            }
        }
    }
    (graph, layout)
}

/// A blend tree's weights for one frame, split by how they combine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeWeights {
    /// Clips blended normally. Sums to 1 (or is empty).
    pub blend: Vec<(String, f32)>,
    /// Difference clips added on top, each at its own weight.
    pub additive: Vec<(String, f32)>,
}

impl TreeWeights {
    /// The most heavily weighted normally blended clip.
    pub fn dominant(&self) -> Option<&str> {
        self.blend
            .iter()
            .fold(None::<(&str, f32)>, |best, (name, w)| match best {
                Some((_, bw)) if bw >= *w => best,
                _ => Some((name.as_str(), *w)),
            })
            .map(|(n, _)| n)
    }
}

/// Resolve a tree against the current parameters.
pub fn resolve_tree(tree: &BlendTree, params: &AnimParams) -> TreeWeights {
    let mut out = TreeWeights::default();
    walk_tree(tree, params, 1.0, false, &mut out);
    out
}

/// Every clip a blend tree plays and its weight, normal and additive together.
/// Kept as the flat view the tests (and simple callers) read.
pub fn resolve_blend_tree_weights(tree: &BlendTree, params: &AnimParams) -> Vec<(String, f32)> {
    let w = resolve_tree(tree, params);
    w.blend.into_iter().chain(w.additive).collect()
}

/// A parameter as a usable number: gameplay can write anything, and a NaN
/// reaching a weight freezes the character with no error.
fn param(params: &AnimParams, name: &str) -> f32 {
    let v = params.get_float(name);
    if v.is_finite() { v } else { 0.0 }
}

fn walk_tree(tree: &BlendTree, params: &AnimParams, scale: f32, additive: bool, out: &mut TreeWeights) {
    let leaf = |name: &str, w: f32, out: &mut TreeWeights| {
        let list = if additive { &mut out.additive } else { &mut out.blend };
        list.push((name.to_string(), w * scale));
    };
    match tree {
        BlendTree::Clip(name) => leaf(name, 1.0, out),
        BlendTree::Lerp { a, b, param: p } => {
            let t = param(params, p).clamp(0.0, 1.0);
            walk_tree(a, params, scale * (1.0 - t), additive, out);
            walk_tree(b, params, scale * t, additive, out);
        }
        BlendTree::Blend1D { param: p, entries } => {
            for (name, w) in blend_1d(entries, param(params, p)) {
                leaf(name, w, out);
            }
        }
        BlendTree::BlendSpace2D { entries, param_x, param_y } => {
            let p = Vec2::new(param(params, param_x), param(params, param_y));
            for (name, w) in blend_2d(entries, p) {
                leaf(name, w, out);
            }
        }
        BlendTree::Additive { base, overlay, param: p } => {
            let amount = param(params, p).clamp(0.0, 1.0);
            walk_tree(base, params, scale, additive, out);
            walk_tree(overlay, params, scale * amount, true, out);
        }
    }
}

/// The weights along a 1D line: the two entries either side of `p` share it.
fn blend_1d(entries: &[crate::blend_tree::Blend1DEntry], p: f32) -> Vec<(&str, f32)> {
    let mut sorted: Vec<&crate::blend_tree::Blend1DEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.threshold.total_cmp(&b.threshold));
    let (Some(first), Some(last)) = (sorted.first(), sorted.last()) else {
        return Vec::new();
    };
    if p <= first.threshold {
        return vec![(first.clip.as_str(), 1.0)];
    }
    if p >= last.threshold {
        return vec![(last.clip.as_str(), 1.0)];
    }
    for pair in sorted.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if p >= a.threshold && p < b.threshold {
            let span = b.threshold - a.threshold;
            let t = if span > f32::EPSILON { (p - a.threshold) / span } else { 1.0 };
            return vec![(a.clip.as_str(), 1.0 - t), (b.clip.as_str(), t)];
        }
    }
    vec![(last.clip.as_str(), 1.0)]
}

/// The weights in a 2D blend space, by gradient band interpolation.
///
/// Inverse distance weighting, which this used before, never gives an entry
/// its clip alone (every other entry keeps a share however far away it is),
/// so a character standing still on the idle entry still carried a few percent
/// of every walk. Gradient bands give each entry weight 1 on itself and 0 at
/// and beyond each neighbour, which is what animators expect from a blend
/// space.
fn blend_2d(entries: &[crate::blend_tree::BlendSpaceEntry], p: Vec2) -> Vec<(&str, f32)> {
    if entries.is_empty() {
        return Vec::new();
    }
    let points: Vec<Vec2> = entries.iter().map(|e| Vec2::new(e.x, e.y)).collect();
    let mut weights: Vec<f32> = Vec::with_capacity(entries.len());
    for (i, pi) in points.iter().enumerate() {
        let mut h = 1.0f32;
        for (j, pj) in points.iter().enumerate() {
            let edge = *pj - *pi;
            let len2 = edge.length_squared();
            if i == j || len2 <= f32::EPSILON {
                continue;
            }
            let band = 1.0 - (p - *pi).dot(edge) / len2;
            h = h.min(band.clamp(0.0, 1.0));
        }
        weights.push(if h.is_finite() { h } else { 0.0 });
    }
    let total: f32 = weights.iter().sum();
    if total <= f32::EPSILON || !total.is_finite() {
        // Every band says no (possible only with coincident entries): play the
        // nearest one rather than nothing.
        let nearest = points
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.distance_squared(p).total_cmp(&b.1.distance_squared(p)))
            .map(|(i, _)| i)
            .unwrap_or(0);
        return vec![(entries[nearest].clip.as_str(), 1.0)];
    }
    entries
        .iter()
        .zip(weights)
        .filter(|(_, w)| *w > 0.0)
        .map(|(e, w)| (e.clip.as_str(), w / total))
        .collect()
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

    // ── 1D blend ─────────────────────────────────────────────────────────────

    fn line() -> BlendTree {
        use crate::blend_tree::Blend1DEntry;
        BlendTree::Blend1D {
            param: "speed".into(),
            // Out of order on purpose: thresholds sort when resolved.
            entries: vec![
                Blend1DEntry { clip: "run".into(), threshold: 4.0 },
                Blend1DEntry { clip: "idle".into(), threshold: 0.0 },
                Blend1DEntry { clip: "walk".into(), threshold: 1.5 },
            ],
        }
    }

    #[test]
    fn a_1d_blend_shares_weight_between_its_neighbours() {
        let w = resolve_blend_tree_weights(&line(), &params(&[("speed", 2.75)]));
        assert!((weight_of(&w, "walk") - 0.5).abs() < 1e-5);
        assert!((weight_of(&w, "run") - 0.5).abs() < 1e-5);
        assert_eq!(weight_of(&w, "idle"), 0.0);
        assert!((total(&w) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_1d_blend_holds_its_ends() {
        let below = resolve_blend_tree_weights(&line(), &params(&[("speed", -3.0)]));
        assert_eq!(below, vec![("idle".to_string(), 1.0)]);
        let above = resolve_blend_tree_weights(&line(), &params(&[("speed", 99.0)]));
        assert_eq!(above, vec![("run".to_string(), 1.0)]);
        let on = resolve_blend_tree_weights(&line(), &params(&[("speed", 1.5)]));
        assert!((weight_of(&on, "walk") - 1.0).abs() < 1e-5);
    }

    /// A NaN parameter (a division by zero in gameplay code) reads as zero.
    #[test]
    fn a_nan_parameter_reads_as_zero() {
        let w = resolve_blend_tree_weights(&line(), &params(&[("speed", f32::NAN)]));
        assert_eq!(w, vec![("idle".to_string(), 1.0)]);
    }

    /// The additive overlay is reported apart from the blend, which is what
    /// routes it to difference clips instead of averaging it in.
    #[test]
    fn an_overlay_resolves_to_the_additive_list() {
        let tree = BlendTree::Additive { base: Box::new(line()), overlay: clip("breathe"), param: "b".into() };
        let w = resolve_tree(&tree, &params(&[("speed", 0.0), ("b", 0.4)]));
        assert_eq!(w.blend, vec![("idle".to_string(), 1.0)]);
        assert_eq!(w.additive, vec![("breathe".to_string(), 0.4)]);
        assert_eq!(w.dominant(), Some("idle"));
    }

    // ── graph layout ────────────────────────────────────────────────────────

    fn handles(names: &[&str]) -> Vec<(String, Handle<AnimationClip>)> {
        names.iter().map(|n| (n.to_string(), Handle::default())).collect()
    }

    fn parent_of(graph: &AnimationGraph, node: AnimationNodeIndex) -> AnimationNodeIndex {
        graph
            .graph
            .edge_indices()
            .filter_map(|e| graph.graph.edge_endpoints(e))
            .find(|(_, to)| *to == node)
            .map(|(from, _)| from)
            .expect("every clip node has a parent")
    }

    /// The editor preview and every base command find a slot through
    /// `layout.base`; each slot must get exactly one base node.
    #[test]
    fn every_slot_gets_one_base_node() {
        let base = handles(&["idle", "walk"]);
        let additive = HashMap::new();
        let (graph, layout) = build_animator_graph(&GraphSpec { base: &base, additive: &additive, overlay_slots: &[], layers: &[] });
        assert_eq!(layout.base.len(), 2);
        assert_eq!(parent_of(&graph, layout.base["idle"]), parent_of(&graph, layout.base["walk"]));
        assert_eq!(layout.roles[&layout.base["walk"]], ("walk".to_string(), NodeRole::Base));
    }

    /// A masked layer masks its own clip nodes and puts every excluded bone
    /// in its group, and nothing else: the base must still move those bones.
    #[test]
    fn a_masked_layer_masks_only_its_own_nodes() {
        let base = handles(&["idle", "wave"]);
        let additive = HashMap::new();
        let legs = crate::bone_target("LeftLeg");
        let layers = vec![
            LayerSpec { mode: LayerBlendMode::Override, masked_out: None },
            LayerSpec { mode: LayerBlendMode::Override, masked_out: Some(vec![legs]) },
        ];
        let (graph, layout) = build_animator_graph(&GraphSpec { base: &base, additive: &additive, overlay_slots: &[], layers: &layers });
        assert_eq!(layout.layers.len(), 2);
        assert_eq!(graph.mask_groups.get(&legs), Some(&0b10));
        let masked = layout.layers[1].nodes["wave"];
        assert_eq!(graph[masked].mask, 0b10);
        assert_eq!(graph[layout.layers[0].nodes["wave"]].mask, 0);
        assert_eq!(graph[layout.base["wave"]].mask, 0);
    }

    /// In an Add node the first child evaluated is what the rest add onto,
    /// so the stack below an additive layer must come before the layer's
    /// clips, and an additive layer plays difference clips.
    #[test]
    fn an_additive_layer_adds_onto_the_stack_below() {
        let base = handles(&["idle", "breathe"]);
        let additive = HashMap::from([("breathe".to_string(), Handle::<AnimationClip>::default())]);
        let layers = vec![LayerSpec { mode: LayerBlendMode::Additive, masked_out: None }];
        let (graph, layout) = build_animator_graph(&GraphSpec { base: &base, additive: &additive, overlay_slots: &[], layers: &layers });
        let layer_node = layout.layers[0].nodes["breathe"];
        assert!(!layout.layers[0].nodes.contains_key("idle"), "no difference clip, no additive node");
        let wrapper = parent_of(&graph, layer_node);
        assert!(matches!(graph[wrapper].node_type, bevy::animation::graph::AnimationNodeType::Add));
        let below = parent_of(&graph, layout.base["idle"]);
        assert_eq!(parent_of(&graph, below), wrapper);
        assert!(below.index() < layer_node.index());
    }

    #[test]
    fn a_tree_overlay_gets_an_additive_base() {
        let base = handles(&["walk", "breathe"]);
        let additive = HashMap::from([("breathe".to_string(), Handle::<AnimationClip>::default())]);
        let overlay = vec!["breathe".to_string()];
        let (graph, layout) = build_animator_graph(&GraphSpec { base: &base, additive: &additive, overlay_slots: &overlay, layers: &[] });
        let over = layout.overlay["breathe"];
        let add = parent_of(&graph, over);
        assert!(matches!(graph[add].node_type, bevy::animation::graph::AnimationNodeType::Add));
        let blend = parent_of(&graph, layout.base["walk"]);
        assert_eq!(parent_of(&graph, blend), add);
        assert!(blend.index() < over.index());
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
