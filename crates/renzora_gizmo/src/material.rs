//! The gizmo's always-on-top material, and the handle-to-material lookup.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GizmoMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub emissive: LinearRgba,
}

impl Material for GizmoMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_gizmo/shaders/gizmo_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            // wgpu 29: these `DepthStencilState` fields are now `Option`.
            depth_stencil.depth_compare = Some(CompareFunction::Always);
            depth_stencil.depth_write_enabled = Some(false);
        }
        // Gizmo meshes get mirrored via negative root scale when axes flip
        // to face the camera — disable backface culling so cone heads and
        // scale cubes keep rendering correctly regardless of winding.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The eight handle materials, built once in `setup_gizmo_meshes` and swapped
/// between by `update_gizmo_materials` as the hover / active axis changes.
#[derive(Resource)]
pub(crate) struct GizmoMaterials {
    pub x_normal: Handle<GizmoMaterial>,
    pub x_highlight: Handle<GizmoMaterial>,
    pub y_normal: Handle<GizmoMaterial>,
    pub y_highlight: Handle<GizmoMaterial>,
    pub z_normal: Handle<GizmoMaterial>,
    pub z_highlight: Handle<GizmoMaterial>,
    pub center_normal: Handle<GizmoMaterial>,
    pub center_highlight: Handle<GizmoMaterial>,
}
