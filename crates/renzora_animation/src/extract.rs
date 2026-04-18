//! Animation extraction utilities — re-exports write_anim_file from renzora.

pub use renzora::{write_anim_file, AnimClip, BoneTrack};

/// Create a minimal AnimClip with the given name and duration.
pub fn create_empty_clip(name: &str, duration: f32) -> AnimClip {
    AnimClip {
        name: name.to_string(),
        duration,
        tracks: Vec::new(),
    }
}

/// Create a BoneTrack with translation keyframes.
pub fn create_translation_track(
    bone_name: &str,
    keyframes: Vec<(f32, [f32; 3])>,
) -> BoneTrack {
    BoneTrack {
        bone_name: bone_name.to_string(),
        translations: keyframes,
        rotations: Vec::new(),
        scales: Vec::new(),
    }
}

/// Create a BoneTrack with rotation keyframes (quaternion XYZW).
pub fn create_rotation_track(
    bone_name: &str,
    keyframes: Vec<(f32, [f32; 4])>,
) -> BoneTrack {
    BoneTrack {
        bone_name: bone_name.to_string(),
        translations: Vec::new(),
        rotations: keyframes,
        scales: Vec::new(),
    }
}
