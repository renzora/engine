//! The cloud dome material: one uniform block plus the two baked noise volumes.
//!
//! The dome is only a way to get a fragment per sky pixel — see `clouds.wgsl`
//! for what actually happens per fragment.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Everything `clouds.wgsl` reads, in the order its `CloudsUniform` declares.
///
/// Lengths are **kilometres**: at a 6371 km planet radius the shader's shell
/// intersections lose the entire cloud deck to f32 rounding if fed metres.
/// `CloudsData` keeps its heights in metres, because that is what a level
/// designer thinks in, and the conversion happens where the uniform is packed.
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct CloudsUniform {
    pub sun_direction: Vec4,
    pub sun_color: Vec4,
    pub ambient_top: Vec4,
    pub ambient_bottom: Vec4,
    /// `rgb` = horizon haze in the sun's half of the sky, `a` = atmosphere
    /// strength.
    pub haze_sunward: Vec4,
    /// `rgb` = horizon haze opposite the sun, `w` unused.
    pub haze_away: Vec4,
    /// `xyz` = accumulated wind displacement in km.
    pub wind_offset: Vec4,
    /// `xy` = the warp field's scroll in km, `z` = the detail volume's phase in
    /// whole turns.
    pub morph_offset: Vec4,

    pub planet_radius: f32,
    pub bottom_height: f32,
    pub top_height: f32,
    pub base_scale: f32,
    pub detail_scale: f32,
    pub coverage: f32,
    /// Extinction per km at full density.
    pub extinction: f32,
    pub detail_strength: f32,
    pub edge_softness: f32,
    pub base_softness: f32,
    pub powder_strength: f32,
    pub min_transmittance: f32,
    pub forward_scattering: f32,
    pub backward_scattering: f32,
    pub scattering_blend: f32,
    pub view_steps: u32,
    pub shadow_steps: u32,
    /// 1 in daylight, 0 at night.
    pub day_factor: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub uniform: CloudsUniform,

    /// Weather/shape atlas — see [`crate::noise`].
    #[texture(1)]
    #[sampler(2)]
    pub base_noise: Handle<Image>,

    /// High-frequency erosion volume.
    #[texture(3, dimension = "3d")]
    #[sampler(4)]
    pub detail_noise: Handle<Image>,
}

impl Material for CloudMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://renzora_clouds/clouds.wgsl".into())
    }

    /// The raymarch accumulates *premultiplied* radiance: each step adds
    /// `transmittance * radiance * (1 - step_transmittance)`, which is already
    /// weighted by how much of the pixel that step covers. Dividing it back out
    /// to straight alpha only to have the blender multiply it in again loses
    /// precision in exactly the thin rim pixels the powder term exists for.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    /// The dome is centred on the camera, so its transparent-sort distance is
    /// ~0 — the nearest item in the phase — making it draw *last* and blend
    /// clouds over every transparent that doesn't write depth (gaussian
    /// splats). Bias the sort distance to -inf so the dome always draws first,
    /// as sky background; it still depth-tests against opaque geometry.
    fn depth_bias(&self) -> f32 {
        f32::NEG_INFINITY
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // A camera inside the deck sees its own dome from within, and a camera
        // above it looks down through the far side. Neither survives backface
        // culling.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
