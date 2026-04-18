//! Animation conversion utilities.
//!
//! Converts UsdAnimation data to renzora_animation::AnimClip format.

use crate::scene::UsdAnimation;
use renzora_animation::clip::{AnimClip, BoneTrack};

/// Convert a UsdAnimation to a renzora AnimClip.
pub fn to_anim_clip(anim: &UsdAnimation) -> AnimClip {
    let mut tracks = Vec::new();

    for jt in &anim.joint_tracks {
        // Extract the joint name from the path (last segment)
        let bone_name = jt
            .joint_path
            .rsplit('/')
            .next()
            .unwrap_or(&jt.joint_path)
            .to_string();

        tracks.push(BoneTrack {
            bone_name,
            translations: jt.translations.clone(),
            rotations: jt.rotations.clone(),
            scales: jt.scales.clone(),
        });
    }

    AnimClip {
        name: anim.name.clone(),
        duration: anim.duration,
        tracks,
    }
}
