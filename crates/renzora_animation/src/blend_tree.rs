//! Blend Trees — hierarchical animation blending.
//!
//! Blend trees compose multiple clips into a single blended output.
//! They are referenced from state machine states via `StateMotion::BlendTree`.

use serde::{Deserialize, Serialize};

/// A blend tree node — recursive structure for composing animations.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 2D blend space — multiple entries placed in a 2D parameter space.
    BlendSpace2D {
        entries: Vec<BlendSpaceEntry>,
        /// Float parameter for X axis.
        param_x: String,
        /// Float parameter for Y axis.
        param_y: String,
    },
    /// Additive blend — overlay on top of a base animation.
    Additive {
        base: Box<BlendTree>,
        overlay: Box<BlendTree>,
        /// Float parameter controlling overlay weight (0.0–1.0).
        param: String,
    },
}

/// An entry in a 2D blend space.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
