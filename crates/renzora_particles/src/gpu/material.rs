//! The GPU backend's two materials: the CPU path's particle materials with
//! the simulation's storage buffers bound beside the uniform and texture.
//!
//! Both draw with the very shaders the CPU path draws with
//! (`particle3d.wesl`, `particle2d.wesl`), under the `GPU_PARTICLES` shader
//! def, which swaps the vertex input for one quad corner and pool slot and
//! fetches the particle from the buffer. The pipelines differ only in that
//! def and the vertex layout, so blending, the deferred and forward routes
//! (the G-buffer pass included, through `deferred_vertex_shader`),
//! soft particles, tonemapping, the prepass, shadows and motion vectors are
//! whatever `crate::render::material` does, for the reasons its module docs
//! give. A GPU emitter wearing a project material draws through
//! `crate::render::surface` instead, with the same fetch.
//!
//! The buffers are `ShaderBuffer` assets the runtime owns (`super::runtime`),
//! which is what lets a Bevy material bind a buffer the compute kernels write:
//! the render world's `GpuShaderBuffer` for the handle is the one buffer both
//! the material's bind group and the kernels' bind group point at.

use bevy::mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use crate::asset::BlendMode;
use crate::render::material::{add_def, ParticleMaterialParams, ATTRIBUTE_PARTICLE_LOCAL, MESH_PARTICLES_DEF, SHADER_2D, SHADER_3D};

/// The shader def that switches the particle shaders to the storage fetch.
pub const GPU_PARTICLES_DEF: &str = "GPU_PARTICLES";

/// The vertex input: corner in xy and pool slot in z, plus, for mesh
/// particles, each vertex's position and normal in the source mesh (see
/// `render::mesh_particles::gpu_pool_mesh`). The particle-surface pipeline
/// reads the same pool mesh.
pub(crate) fn pool_vertex_layout(
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
) -> Result<VertexBufferLayout, SpecializedMeshPipelineError> {
    let mut attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
    if mesh {
        attributes.push(ATTRIBUTE_PARTICLE_LOCAL.at_shader_location(1));
        attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(2));
    }
    Ok(layout.0.get_layout(&attributes)?)
}

fn apply_common(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    mesh: bool,
) -> Result<(), SpecializedMeshPipelineError> {
    descriptor.vertex.buffers = vec![pool_vertex_layout(layout, mesh)?];
    add_def(descriptor, GPU_PARTICLES_DEF);
    if mesh {
        add_def(descriptor, MESH_PARTICLES_DEF);
    }
    descriptor.primitive.cull_mode = None;
    Ok(())
}

#[cfg(feature = "render_3d")]
pub use three::GpuParticleMaterial3d;

#[cfg(feature = "render_3d")]
mod three {
    use super::*;
    use crate::render::material::{alpha_mode_3d, Particle3dKey, ParticleRouteMut, RenderRoute};
    use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};

    #[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
    #[bind_group_data(Particle3dKey)]
    pub struct GpuParticleMaterial3d {
        #[uniform(0)]
        pub params: ParticleMaterialParams,
        #[texture(1)]
        #[sampler(2)]
        pub texture: Option<Handle<Image>>,
        #[storage(3, read_only, visibility(vertex))]
        pub particles: Handle<ShaderBuffer>,
        #[storage(4, read_only, visibility(vertex))]
        pub state: Handle<ShaderBuffer>,
        pub blend: BlendMode,
        /// [`BlendMode::AlphaMask`]'s cutoff, for the pipeline's alpha mode.
        pub cutoff: f32,
        /// Draws mesh particles from a pool mesh.
        pub mesh: bool,
        /// Forward or the G-buffer, as on the CPU path.
        pub route: RenderRoute,
    }

    impl From<&GpuParticleMaterial3d> for Particle3dKey {
        fn from(material: &GpuParticleMaterial3d) -> Self {
            Self { mesh: material.mesh, method: material.route.method }
        }
    }

    impl ParticleRouteMut for GpuParticleMaterial3d {
        fn route(&self) -> RenderRoute {
            self.route
        }
        fn set_route(&mut self, route: RenderRoute) {
            self.route = route;
        }
    }

    impl crate::render::ParticleParamsMut for GpuParticleMaterial3d {
        fn particle_params_mut(&mut self) -> &mut ParticleMaterialParams {
            &mut self.params
        }
    }

    impl Material for GpuParticleMaterial3d {
        fn vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn prepass_vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn prepass_fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        // The G-buffer pass is the prepass pipeline too, and needs the
        // storage fetch as much as the others do.
        fn deferred_vertex_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn deferred_fragment_shader() -> ShaderRef {
            SHADER_3D.into()
        }

        fn alpha_mode(&self) -> AlphaMode {
            alpha_mode_3d(self.blend, self.cutoff)
        }

        fn opaque_render_method(&self) -> bevy::material::OpaqueRendererMethod {
            self.route.opaque()
        }

        fn enable_oit() -> bool {
            false
        }

        fn specialize(
            _pipeline: &MaterialPipeline,
            descriptor: &mut RenderPipelineDescriptor,
            layout: &MeshVertexBufferLayoutRef,
            key: MaterialPipelineKey<Self>,
        ) -> Result<(), SpecializedMeshPipelineError> {
            apply_common(descriptor, layout, key.bind_group_data.mesh)
        }
    }
}

#[cfg(feature = "render_2d")]
pub use two::GpuParticleMaterial2d;

#[cfg(feature = "render_2d")]
mod two {
    use super::*;
    use crate::render::material::Particle2dKey;
    use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline};

    #[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
    #[bind_group_data(Particle2dKey)]
    pub struct GpuParticleMaterial2d {
        #[uniform(0)]
        pub params: ParticleMaterialParams,
        #[texture(1)]
        #[sampler(2)]
        pub texture: Option<Handle<Image>>,
        #[storage(3, read_only, visibility(vertex))]
        pub particles: Handle<ShaderBuffer>,
        #[storage(4, read_only, visibility(vertex))]
        pub state: Handle<ShaderBuffer>,
        pub blend: BlendMode,
        /// [`BlendMode::AlphaMask`]'s cutoff, for the pipeline's alpha mode.
        pub cutoff: f32,
    }

    impl From<&GpuParticleMaterial2d> for Particle2dKey {
        fn from(material: &GpuParticleMaterial2d) -> Self {
            Self::of(material.blend)
        }
    }

    impl Material2d for GpuParticleMaterial2d {
        fn vertex_shader() -> ShaderRef {
            SHADER_2D.into()
        }

        fn fragment_shader() -> ShaderRef {
            SHADER_2D.into()
        }

        fn alpha_mode(&self) -> AlphaMode2d {
            crate::render::material::alpha_mode_2d(self.blend, self.cutoff)
        }

        fn specialize(
            _pipeline: &Material2dPipeline,
            descriptor: &mut RenderPipelineDescriptor,
            layout: &MeshVertexBufferLayoutRef,
            key: Material2dKey<Self>,
        ) -> Result<(), SpecializedMeshPipelineError> {
            apply_common(descriptor, layout, false)?;
            crate::render::material::apply_blend_2d(descriptor, key.bind_group_data);
            Ok(())
        }
    }
}
