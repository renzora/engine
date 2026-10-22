//! The overlay material: the reflecting surface drawn again on top of itself,
//! showing the mirror camera's image.
//!
//! An overlay rather than a replacement because the surface's own material has
//! to keep working. A window is glass with a reflection on it, a car door is
//! metal with one; swapping their material for a "mirror material" would throw
//! away exactly what makes them that surface. So the user's material renders as
//! it always did, and this one blends the reflection over it with the strength,
//! tint, fresnel and blur from [`crate::PlanarReflection`].
//!
//! It is transparent, so it goes to the forward transparent pass even when the
//! camera is deferred, and it has no prepass, no shadows and no OIT: it adds
//! light, it occupies no space.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub const OVERLAY_SHADER: &str =
    "embedded://renzora_planar_reflection/shaders/reflection_overlay.wesl";

/// The uniform the overlay shader reads. Layout matches `OverlayParams` in
/// `reflection_overlay.wesl`.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct OverlayParams {
    /// Linear RGB multiplied into the reflection.
    pub tint: Vec4,
    /// `x` strength, `y` fresnel, `z` blur.
    pub shading: Vec4,
    /// World-space normal of the reflecting side.
    pub normal: Vec4,
    /// World position of the camera this overlay was captured for, `w` the
    /// tolerance within which a view counts as that camera.
    pub view_origin: Vec4,
    /// That camera's forward direction.
    pub view_forward: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ReflectionOverlayMaterial {
    #[uniform(0)]
    pub params: OverlayParams,
    #[texture(1)]
    #[sampler(2)]
    pub reflection: Handle<Image>,
}

impl Material for ReflectionOverlayMaterial {
    fn vertex_shader() -> ShaderRef {
        OVERLAY_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        OVERLAY_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn enable_oit() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The shader reads positions and nothing else, so it works on any mesh
        // a user puts the component on, with or without normals or UVs.
        descriptor.vertex.buffers = vec![layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?];
        Ok(())
    }
}
