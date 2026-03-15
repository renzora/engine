//! GLTF animation extraction — writes `.anim` RON files from AnimClip data.
//!
//! Provides utilities for creating `.anim` files from animation data.
//! The actual GLTF parsing and clip extraction is done via Bevy's asset loading;
//! this module provides the serialization and file-writing side.

use crate::clip::{AnimClip, BoneTrack};

/// Write an AnimClip to a RON file at the given path.
pub fn write_anim_file(clip: &AnimClip, path: &std::path::Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(clip, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialization error: {}", e))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(path, ron_str)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Create a minimal AnimClip with the given name and duration.
/// Tracks can be added afterward.
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
