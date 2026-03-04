//! .anim file format — RON-serializable animation clip data.

use serde::{Deserialize, Serialize};

/// One animation clip, serialized to a `.anim` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimFile {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
}

/// Animation curves for one bone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneTrack {
    /// Normalized bone name (no "mixamorig:" prefix).
    pub bone_name: String,
    /// (time_sec, [x, y, z])
    pub translations: Vec<(f32, [f32; 3])>,
    /// (time_sec, [x, y, z, w]) — quaternion
    pub rotations: Vec<(f32, [f32; 4])>,
    /// (time_sec, [x, y, z])
    pub scales: Vec<(f32, [f32; 3])>,
}
