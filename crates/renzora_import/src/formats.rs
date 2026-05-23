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
    &[
        "gltf", "glb", "obj", "stl", "ply", "fbx", "usd", "usda", "usdc", "usdz", "abc", "dae",
        "bvh", "blend",
    ]
}

/// Check if a file path has a supported 3D model extension.
pub fn is_supported(path: &Path) -> bool {
    detect_format(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detect_format_known_extensions() {
        assert_eq!(detect_format(Path::new("a.obj")), Some(ModelFormat::Obj));
        assert_eq!(detect_format(Path::new("a.gltf")), Some(ModelFormat::Gltf));
        assert_eq!(detect_format(Path::new("a.glb")), Some(ModelFormat::Glb));
        assert_eq!(detect_format(Path::new("a.stl")), Some(ModelFormat::Stl));
        assert_eq!(detect_format(Path::new("a.ply")), Some(ModelFormat::Ply));
        assert_eq!(detect_format(Path::new("a.fbx")), Some(ModelFormat::Fbx));
        assert_eq!(detect_format(Path::new("a.usdz")), Some(ModelFormat::Usdz));
        assert_eq!(detect_format(Path::new("a.abc")), Some(ModelFormat::Abc));
        assert_eq!(detect_format(Path::new("a.dae")), Some(ModelFormat::Dae));
        assert_eq!(detect_format(Path::new("a.bvh")), Some(ModelFormat::Bvh));
        assert_eq!(detect_format(Path::new("a.blend")), Some(ModelFormat::Blend));
    }

    #[test]
    fn detect_format_usd_variants_all_map_to_usd() {
        assert_eq!(detect_format(Path::new("a.usd")), Some(ModelFormat::Usd));
        assert_eq!(detect_format(Path::new("a.usda")), Some(ModelFormat::Usd));
        assert_eq!(detect_format(Path::new("a.usdc")), Some(ModelFormat::Usd));
    }

    #[test]
    fn detect_format_is_case_insensitive() {
        assert_eq!(detect_format(Path::new("Model.OBJ")), Some(ModelFormat::Obj));
        assert_eq!(detect_format(Path::new("Model.Fbx")), Some(ModelFormat::Fbx));
    }

    #[test]
    fn detect_format_uses_full_path() {
        assert_eq!(
            detect_format(Path::new("/some/dir/scene.glb")),
            Some(ModelFormat::Glb)
        );
    }

    #[test]
    fn detect_format_unknown_and_missing_extension() {
        assert_eq!(detect_format(Path::new("a.txt")), None);
        assert_eq!(detect_format(Path::new("noextension")), None);
        assert_eq!(detect_format(Path::new("a.")), None);
    }

    #[test]
    fn supported_extensions_match_detect_format() {
        // Every advertised extension must actually be detected.
        for ext in supported_extensions() {
            let name = format!("file.{}", ext);
            assert!(
                detect_format(Path::new(&name)).is_some(),
                "extension {} advertised as supported but not detected",
                ext
            );
        }
        // And a known-bad one must not be in the list.
        assert!(!supported_extensions().contains(&"txt"));
    }

    #[test]
    fn is_supported_matches_detect_format() {
        assert!(is_supported(Path::new("a.obj")));
        assert!(!is_supported(Path::new("a.txt")));
        assert!(!is_supported(Path::new("noext")));
    }

    #[test]
    fn display_name_is_stable() {
        assert_eq!(ModelFormat::Obj.display_name(), "OBJ (Wavefront)");
        assert_eq!(ModelFormat::Glb.display_name(), "GLB");
    }
}
