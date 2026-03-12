//! Import settings that control how models are converted to GLB.

/// Axis convention for the up direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpAxis {
    /// Detect from the source file (default).
    Auto,
    /// Y is up (Bevy / GLTF convention).
    YUp,
    /// Z is up (Blender default, many CAD tools).
    ZUp,
}

/// Settings that control model import and GLB conversion.
#[derive(Debug, Clone)]
pub struct ImportSettings {
    /// Uniform scale factor applied to all geometry.
    pub scale: f32,
    /// Up-axis convention.
    pub up_axis: UpAxis,
    /// Flip the V texture coordinate (1.0 - v).
    pub flip_uvs: bool,
    /// Generate flat normals if the source has none.
    pub generate_normals: bool,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            up_axis: UpAxis::Auto,
            flip_uvs: false,
            generate_normals: true,
        }
    }
}
