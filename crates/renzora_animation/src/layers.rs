//! Animation Layers: stacked animation controllers with masks and blend modes.
//!
//! Layers stack above the base motion (the playing clip, crossfade or state
//! machine state), in list order. Each one plays its own clip
//! ([`AnimationLayer::current_clip`]) at its own weight, either replacing the
//! pose below it (`Override`) or adding its clip's motion on top of it
//! (`Additive`), and a mask restricts it to part of the skeleton.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// How a layer combines with layers below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Reflect)]
pub enum LayerBlendMode {
    /// Replace lower-layer values for masked bones, blended by the weight.
    #[default]
    Override,
    /// Add on top of lower-layer values. The layer's clip plays as a
    /// difference from its own first frame, so a clip that starts in the pose
    /// it was keyed from adds exactly the motion that was keyed.
    Additive,
}

/// Definition of a single animation layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default, Reflect)]
pub struct AnimationLayer {
    /// Layer name (e.g. "upper_body", "face").
    pub name: String,
    /// Layer weight (0.0 to 1.0).
    pub weight: f32,
    /// Avatar mask: the bones this layer may move, as subtree roots. `Spine1`
    /// takes that bone and everything under it; an entry starting with `!`
    /// carves a subtree back out (`!LeftShoulder`), and the deepest entry on a
    /// bone's path wins. `None` (or an empty list) affects every bone.
    ///
    /// Names match the way clips bind to bones, so `mixamorig:Spine1` and
    /// `mixamorig_spine1` are the same entry.
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

    /// The mask entries, or `None` when the layer affects every bone.
    pub fn active_mask(&self) -> Option<&[String]> {
        self.mask.as_deref().filter(|m| !m.is_empty())
    }
}

/// Whether a bone is inside a mask.
///
/// `path` is the bone's own name followed by its ancestors' names, nearest
/// first. The first (deepest) entry found along it decides; a bone under no
/// entry is outside the mask.
pub fn mask_includes(mask: &[String], path: &[&str]) -> bool {
    for bone in path {
        let key = renzora::sanitize_id(bone);
        for entry in mask {
            let (exclude, name) = match entry.strip_prefix('!') {
                Some(rest) => (true, rest),
                None => (false, entry.as_str()),
            };
            if renzora::sanitize_id(name.trim()) == key {
                return !exclude;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|s| s.to_string()).collect()
    }

    /// An upper-body layer takes the spine and everything below it.
    #[test]
    fn an_entry_takes_its_whole_subtree() {
        let m = mask(&["mixamorig:Spine1"]);
        assert!(mask_includes(&m, &["mixamorig:LeftHand", "mixamorig:LeftArm", "mixamorig:Spine1", "mixamorig:Hips"]));
        assert!(!mask_includes(&m, &["mixamorig:LeftLeg", "mixamorig:LeftUpLeg", "mixamorig:Hips"]));
    }

    /// The deepest entry wins, so a subtree can be carved back out.
    #[test]
    fn an_exclusion_carves_a_subtree_back_out() {
        let m = mask(&["Spine1", "!LeftArm"]);
        assert!(!mask_includes(&m, &["LeftHand", "LeftArm", "Spine1"]));
        assert!(mask_includes(&m, &["RightHand", "RightArm", "Spine1"]));
        assert!(mask_includes(&m, &["Spine1"]));
    }

    /// Both spellings of a bone name match, as they do for clip binding.
    #[test]
    fn entries_match_both_spellings() {
        let m = mask(&["mixamorig_head"]);
        assert!(mask_includes(&m, &["mixamorig:Head"]));
    }

    #[test]
    fn an_empty_mask_is_no_mask() {
        assert!(AnimationLayer::new("l").with_mask(vec![]).active_mask().is_none());
        assert!(AnimationLayer::new("l").with_mask(mask(&["Head"])).active_mask().is_some());
    }
}
