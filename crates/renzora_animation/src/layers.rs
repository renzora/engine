//! Animation Layers — stacked animation controllers with masks and blend modes.
//!
//! Each layer can override or additively blend on top of the base layer.
//! Bone masks restrict which bones a layer affects.

use serde::{Deserialize, Serialize};
use bevy::prelude::*;

/// How a layer combines with layers below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Reflect)]
pub enum LayerBlendMode {
    /// Replace lower-layer values for masked bones.
    #[default]
    Override,
    /// Add on top of lower-layer values.
    Additive,
}

/// Definition of a single animation layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default, Reflect)]
pub struct AnimationLayer {
    /// Layer name (e.g. "base", "upper_body", "face").
    pub name: String,
    /// Layer weight (0.0–1.0).
    pub weight: f32,
    /// Bone mask — if Some, only these bones are affected.
    /// If None, all bones are affected.
    pub mask: Option<Vec<String>>,
    /// How this layer blends with layers below.
    pub blend_mode: LayerBlendMode,
    /// Clip slot name currently playing on this layer.
    pub current_clip: Option<String>,
}

impl AnimationLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weight: 1.0,
            mask: None,
            blend_mode: LayerBlendMode::Override,
            current_clip: None,
        }
    }

    pub fn with_mask(mut self, bones: Vec<String>) -> Self {
        self.mask = Some(bones);
        self
    }

    pub fn with_blend_mode(mut self, mode: LayerBlendMode) -> Self {
        self.blend_mode = mode;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }
}
