//! Format detection and supported extension lists.

use std::path::Path;

/// Supported 3D model formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    Gltf,
    Glb,
    Obj,
    Stl,
    Ply,
    Fbx,
    Usd,
    Usdz,
    Abc,
    Dae,
    Bvh,
    Blend,
}

impl ModelFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Gltf => "GLTF",
            Self::Glb => "GLB",
            Self::Obj => "OBJ (Wavefront)",
            Self::Stl => "STL",
            Self::Ply => "PLY",
            Self::Fbx => "FBX (Autodesk)",
            Self::Usd => "USD (Universal Scene Description)",
            Self::Usdz => "USDZ (Universal Scene Description)",
            Self::Abc => "ABC (Alembic)",
            Self::Dae => "DAE (Collada)",
            Self::Bvh => "BVH (Motion Capture)",
            Self::Blend => "Blend (Blender)",
        }
    }
}

/// Detect the model format from a file path extension.
pub fn detect_format(path: &Path) -> Option<ModelFormat> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "gltf" => Some(ModelFormat::Gltf),
        "glb" => Some(ModelFormat::Glb),
        "obj" => Some(ModelFormat::Obj),
        "stl" => Some(ModelFormat::Stl),
        "ply" => Some(ModelFormat::Ply),
        "fbx" => Some(ModelFormat::Fbx),
        "usd" | "usda" | "usdc" => Some(ModelFormat::Usd),
        "usdz" => Some(ModelFormat::Usdz),
        "abc" => Some(ModelFormat::Abc),
        "dae" => Some(ModelFormat::Dae),
        "bvh" => Some(ModelFormat::Bvh),
        "blend" => Some(ModelFormat::Blend),
        _ => None,
    }
}

/// Returns the list of supported file extensions (without dots).
pub fn supported_extensions() -> &'static [&'static str] {
    &["gltf", "glb", "obj", "stl", "ply", "fbx", "usd", "usda", "usdc", "usdz", "abc", "dae", "bvh", "blend"]
}

/// Check if a file path has a supported 3D model extension.
pub fn is_supported(path: &Path) -> bool {
    detect_format(path).is_some()
}
