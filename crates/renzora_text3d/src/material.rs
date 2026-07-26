//! The SDF text material — an unlit, alpha-blended material that samples the
//! signed-distance-field glyph atlas and smoothsteps the edge (see
//! `sdf_text.wgsl`). Swapped in for `StandardMaterial` so 3D text stays crisp
//! when the camera moves close, instead of magnifying a bitmap.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SdfTextMaterial {
    /// Text tint (the atlas is a distance field, so all colour comes from here).
    #[uniform(0)]
    pub color: LinearRgba,
    /// The per-text SDF strip the glyph quads sample.
    #[texture(1)]
    #[sampler(2)]
    pub atlas: Handle<Image>,
}

impl Material for SdfTextMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://renzora_text3d/sdf_text.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
