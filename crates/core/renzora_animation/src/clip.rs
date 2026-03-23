//! `.anim` file format — RON-serializable animation clip data.

use serde::{Deserialize, Serialize};

/// One animation clip, serialized to a `.anim` file (RON format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneTrack {
    /// Bone name matching the entity `Name` in the skeleton hierarchy.
    pub bone_name: String,
    /// (time_sec, [x, y, z]) translation keyframes.
    pub translations: Vec<(f32, [f32; 3])>,
    /// (time_sec, [x, y, z, w]) rotation keyframes (quaternion).
    pub rotations: Vec<(f32, [f32; 4])>,
    /// (time_sec, [x, y, z]) scale keyframes.
    pub scales: Vec<(f32, [f32; 3])>,
}
