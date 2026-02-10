//! Splatmap blending material for surface painting.

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// UUID handle for the dynamically generated splatmap fragment shader.
/// The shader_gen system inserts generated WGSL at this handle for hot-reload.
pub const SPLATMAP_FRAG_SHADER_HANDLE: Handle<Shader> = Handle::Uuid(
    bevy::asset::uuid::Uuid::from_u128(0x501A_70AF_F4A6_5BAD_E400_0000_0000_0001),
    std::marker::PhantomData,
);

/// GPU material that blends up to 4 layers via a splatmap weight texture.
///
/// Each uniform gets its own binding index, matching the proven CloudMaterial pattern.
/// WGSL side: each field maps to `@group(3) @binding(N)` as a separate `var<uniform>`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SplatmapMaterial {
    /// Layer 0 color (rgba)
    #[uniform(0)]
    pub layer_colors_0: Vec4,
    /// Layer 1 color (rgba)
    #[uniform(1)]
    pub layer_colors_1: Vec4,
    /// Layer 2 color (rgba)
    #[uniform(2)]
    pub layer_colors_2: Vec4,
    /// Layer 3 color (rgba)
    #[uniform(3)]
    pub layer_colors_3: Vec4,
    /// Layer 0 properties: (metallic, roughness, uv_scale_x, uv_scale_y)
    #[uniform(4)]
    pub layer_props_0: Vec4,
    /// Layer 1 properties
    #[uniform(5)]
    pub layer_props_1: Vec4,
    /// Layer 2 properties
    #[uniform(6)]
    pub layer_props_2: Vec4,
    /// Layer 3 properties
    #[uniform(7)]
    pub layer_props_3: Vec4,

    /// RGBA splatmap weight texture.
    #[texture(8)]
    #[sampler(9)]
    pub splatmap: Handle<Image>,
}

impl Material for SplatmapMaterial {
    fn fragment_shader() -> ShaderRef {
        SPLATMAP_FRAG_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
