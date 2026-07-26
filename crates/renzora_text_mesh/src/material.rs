//! The SDF text material — an unlit, alpha-blended material that samples the
//! signed-distance-field glyph atlas and smoothsteps the edge (see
//! `sdf_text.wgsl`). Swapped in for `StandardMaterial` so text stays crisp when
//! the camera moves close, instead of magnifying a bitmap.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SdfTextMaterial {
    /// Text tint (the atlas is a distance field, so all colour comes from here —
    /// multiplied by per-vertex colour in the shader when the mesh carries it).
    #[uniform(0)]
    pub color: LinearRgba,
    /// The per-text SDF strip the glyph quads sample.
    #[texture(1)]
    #[sampler(2)]
    pub atlas: Handle<Image>,
}

impl Material for SdfTextMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://renzora_text_mesh/sdf_text.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Register [`SdfTextMaterial`] + its embedded shader exactly once per `App`.
///
/// Both `renzora_text3d` and the world-space UI emitter call this from their own
/// `build()`. The guard makes any call after the first a no-op, so adding both
/// doesn't double-register the `MaterialPlugin` (which would panic). Because this
/// lives in a shared rlib, `SdfTextMaterial` has one stable `TypeId` across the
/// binary and the plugin cdylib, so `is_plugin_added` genuinely dedupes them.
pub fn ensure_sdf_material(app: &mut App) {
    if app.is_plugin_added::<MaterialPlugin<SdfTextMaterial>>() {
        return;
    }
    bevy::asset::embedded_asset!(app, "sdf_text.wgsl");
    app.add_plugins(MaterialPlugin::<SdfTextMaterial>::default());
}
