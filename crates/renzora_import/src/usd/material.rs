//! Material processing utilities.

// Material types and extraction logic are in scene.rs (UsdMaterial)
// and the USDA/USDC parsers. This module provides conversion helpers.

use super::scene::UsdMaterial;

/// Convert a UsdPreviewSurface material to PBR parameters suitable for
/// a standard metallic-roughness workflow (glTF compatible).
pub struct PbrParams {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub normal_scale: f32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub ior: f32,
}

pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl From<&UsdMaterial> for PbrParams {
    fn from(mat: &UsdMaterial) -> Self {
        let alpha_mode = if mat.opacity < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };

        PbrParams {
            base_color: [
                mat.diffuse_color[0],
                mat.diffuse_color[1],
                mat.diffuse_color[2],
                mat.opacity,
            ],
            metallic: mat.metallic,
            roughness: mat.roughness,
            emissive: mat.emissive_color,
            normal_scale: mat.normal_scale,
            alpha_mode,
            alpha_cutoff: 0.5,
            ior: mat.ior,
        }
    }
}
