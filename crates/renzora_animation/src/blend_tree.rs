//! Blend Trees: hierarchical animation blending.
//!
//! Blend trees compose multiple clips into a single blended output. A state
//! machine state plays one through `StateMotion::Blend`, stored inline in the
//! state so a `.animsm` file is self-contained. The weights are resolved every
//! frame from the state machine's parameters by
//! [`crate::graph_builder::resolve_tree`], and the runtime applies them to the
//! clip nodes of the animator's graph.

use serde::{Deserialize, Serialize};

/// A blend tree node: recursive structure for composing animations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlendTree {
    /// Play a single clip by slot name.
    Clip(String),
    /// Linear blend between two children, driven by a float parameter.
    Lerp {
        a: Box<BlendTree>,
        b: Box<BlendTree>,
        /// Name of the float parameter (0.0 = fully A, 1.0 = fully B).
        param: String,
    },
    /// 1D blend: clips placed along one parameter at increasing thresholds
    /// (idle at 0, walk at 1.5, run at 4 for a `speed` parameter). The two
    /// entries either side of the parameter share the weight; past either end
    /// the end entry plays alone.
    ///
    /// The threshold is in the parameter's own units rather than `0..1`, so
    /// gameplay can feed raw velocity straight in, which is what a `Lerp`
    /// chain cannot do without a remap per segment.
    Blend1D {
        /// Float parameter that picks the position along the line.
        param: String,
        /// Entries in any order; they are sorted by threshold when resolved.
        #[serde(default)]
        entries: Vec<Blend1DEntry>,
    },
    /// 2D blend space: multiple entries placed in a 2D parameter space.
    BlendSpace2D {
        entries: Vec<BlendSpaceEntry>,
        /// Float parameter for X axis.
        param_x: String,
        /// Float parameter for Y axis.
        param_y: String,
    },
    /// Additive blend: overlay on top of a base animation.
    ///
    /// The overlay plays as a difference from its own first frame, added onto
    /// whatever the base resolves to, so a breathing or recoil clip layers
    /// onto any locomotion without replacing it.
    Additive {
        base: Box<BlendTree>,
        overlay: Box<BlendTree>,
        /// Float parameter controlling overlay weight (0.0 to 1.0).
        param: String,
    },
}

/// An entry on a [`BlendTree::Blend1D`] line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blend1DEntry {
    /// Clip slot name.
    pub clip: String,
    /// Parameter value at which this clip plays alone.
    pub threshold: f32,
}

/// An entry in a 2D blend space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceEntry {
    /// Clip slot name.
    pub clip: String,
    /// Position in the 2D parameter space.
    pub x: f32,
    pub y: f32,
}

impl BlendTree {
    /// Collect all clip names referenced by this blend tree.
    pub fn collect_clips(&self, out: &mut Vec<String>) {
        match self {
            BlendTree::Clip(name) => out.push(name.clone()),
            BlendTree::Lerp { a, b, .. } => {
                a.collect_clips(out);
                b.collect_clips(out);
            }
            BlendTree::Blend1D { entries, .. } => {
                for entry in entries {
                    out.push(entry.clip.clone());
                }
            }
            BlendTree::BlendSpace2D { entries, .. } => {
                for entry in entries {
                    out.push(entry.clip.clone());
                }
            }
            BlendTree::Additive { base, overlay, .. } => {
                base.collect_clips(out);
                overlay.collect_clips(out);
            }
        }
    }

    /// Clips this tree plays *additively* (anywhere under an `Additive`
    /// overlay). The graph builder gives each a difference clip and an additive
    /// node, which it would otherwise not build.
    pub fn collect_additive_clips(&self, out: &mut Vec<String>) {
        match self {
            BlendTree::Clip(_) | BlendTree::Blend1D { .. } | BlendTree::BlendSpace2D { .. } => {}
            BlendTree::Lerp { a, b, .. } => {
                a.collect_additive_clips(out);
                b.collect_additive_clips(out);
            }
            BlendTree::Additive { base, overlay, .. } => {
                base.collect_additive_clips(out);
                overlay.collect_clips(out);
            }
        }
    }

    /// Every float parameter the tree reads, for the editor to list.
    pub fn collect_params(&self, out: &mut Vec<String>) {
        match self {
            BlendTree::Clip(_) => {}
            BlendTree::Lerp { a, b, param } => {
                out.push(param.clone());
                a.collect_params(out);
                b.collect_params(out);
            }
            BlendTree::Blend1D { param, .. } => out.push(param.clone()),
            BlendTree::BlendSpace2D { param_x, param_y, .. } => {
                out.push(param_x.clone());
                out.push(param_y.clone());
            }
            BlendTree::Additive { base, overlay, param } => {
                out.push(param.clone());
                base.collect_params(out);
                overlay.collect_params(out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str) -> Box<BlendTree> {
        Box::new(BlendTree::Clip(name.to_string()))
    }

    fn clips_of(tree: &BlendTree) -> Vec<String> {
        let mut out = Vec::new();
        tree.collect_clips(&mut out);
        out
    }

    /// `collect_clips` is what decides which clips get loaded and added to the
    /// animation graph. A clip it misses is simply absent at runtime — the
    /// character blends toward an animation that was never loaded, which shows
    /// up as a T-pose rather than as an error.
    #[test]
    fn a_single_clip_collects_itself() {
        assert_eq!(clips_of(&BlendTree::Clip("idle".into())), vec!["idle"]);
    }

    #[test]
    fn a_lerp_collects_both_sides() {
        let tree = BlendTree::Lerp {
            a: clip("walk"),
            b: clip("run"),
            param: "speed".into(),
        };
        assert_eq!(clips_of(&tree), vec!["walk", "run"]);
    }

    #[test]
    fn a_blend_space_collects_every_entry() {
        let tree = BlendTree::BlendSpace2D {
            entries: vec![
                BlendSpaceEntry { clip: "idle".into(), x: 0.0, y: 0.0 },
                BlendSpaceEntry { clip: "fwd".into(), x: 0.0, y: 1.0 },
                BlendSpaceEntry { clip: "left".into(), x: -1.0, y: 0.0 },
            ],
            param_x: "x".into(),
            param_y: "y".into(),
        };
        assert_eq!(clips_of(&tree), vec!["idle", "fwd", "left"]);
    }

    #[test]
    fn an_additive_collects_base_and_overlay() {
        let tree = BlendTree::Additive {
            base: clip("run"),
            overlay: clip("wave"),
            param: "wave_amount".into(),
        };
        assert_eq!(clips_of(&tree), vec!["run", "wave"]);
    }

    /// Trees nest arbitrarily, and the recursion is the part a refactor breaks.
    #[test]
    fn nesting_is_walked_to_the_leaves() {
        let locomotion = BlendTree::Lerp {
            a: Box::new(BlendTree::Lerp {
                a: clip("idle"),
                b: clip("walk"),
                param: "speed".into(),
            }),
            b: clip("run"),
            param: "speed".into(),
        };
        let tree = BlendTree::Additive {
            base: Box::new(locomotion),
            overlay: Box::new(BlendTree::BlendSpace2D {
                entries: vec![BlendSpaceEntry { clip: "aim".into(), x: 0.0, y: 0.0 }],
                param_x: "aim_x".into(),
                param_y: "aim_y".into(),
            }),
            param: "aiming".into(),
        };
        assert_eq!(clips_of(&tree), vec!["idle", "walk", "run", "aim"]);
    }

    /// An empty blend space is authorable in the editor (add the node, add no
    /// entries) and must not contribute a phantom clip.
    #[test]
    fn an_empty_blend_space_collects_nothing() {
        let tree = BlendTree::BlendSpace2D {
            entries: vec![],
            param_x: "x".into(),
            param_y: "y".into(),
        };
        assert!(clips_of(&tree).is_empty());
    }

    /// The same clip reachable twice is reported twice — the caller dedupes.
    /// Documented because a future "fix" that dedupes here would change what the
    /// graph builder receives.
    #[test]
    fn a_clip_used_twice_is_reported_twice() {
        let tree = BlendTree::Lerp {
            a: clip("idle"),
            b: clip("idle"),
            param: "t".into(),
        };
        assert_eq!(clips_of(&tree), vec!["idle", "idle"]);
    }

    #[test]
    fn collecting_appends_rather_than_replaces() {
        let mut out = vec!["existing".to_string()];
        BlendTree::Clip("added".into()).collect_clips(&mut out);
        assert_eq!(out, vec!["existing", "added"]);
    }

    #[test]
    fn a_1d_blend_collects_every_entry() {
        let tree = BlendTree::Blend1D {
            param: "speed".into(),
            entries: vec![
                Blend1DEntry { clip: "idle".into(), threshold: 0.0 },
                Blend1DEntry { clip: "run".into(), threshold: 4.0 },
            ],
        };
        assert_eq!(clips_of(&tree), vec!["idle", "run"]);
    }

    /// Only clips under an overlay are additive: the base of an `Additive`
    /// blends normally, and needs no difference clip.
    #[test]
    fn only_overlay_clips_are_additive() {
        let tree = BlendTree::Additive {
            base: Box::new(BlendTree::Lerp { a: clip("walk"), b: clip("run"), param: "s".into() }),
            overlay: clip("breathe"),
            param: "b".into(),
        };
        let mut out = Vec::new();
        tree.collect_additive_clips(&mut out);
        assert_eq!(out, vec!["breathe"]);
    }

    /// The `.animsm` format stores trees inline, so they must survive RON.
    #[test]
    fn a_tree_round_trips_through_ron() {
        let tree = BlendTree::Blend1D {
            param: "speed".into(),
            entries: vec![Blend1DEntry { clip: "walk".into(), threshold: 1.5 }],
        };
        let text = ron::ser::to_string(&tree).unwrap();
        let back: BlendTree = ron::from_str(&text).unwrap();
        assert_eq!(back, tree);
    }
}
