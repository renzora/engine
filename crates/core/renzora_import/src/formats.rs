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
        _ => None,
    }
}

/// Returns the list of supported file extensions (without dots).
pub fn supported_extensions() -> &'static [&'static str] {
    &["gltf", "glb", "obj", "stl", "ply", "fbx"]
}

/// Check if a file path has a supported 3D model extension.
pub fn is_supported(path: &Path) -> bool {
    detect_format(path).is_some()
}
