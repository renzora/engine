//! Custom material used by viewport visualization modes.
//!
//! One [`ViewportDebugMaterial`] is created per source [`StandardMaterial`]
//! so that we can carry the original metallic-roughness texture into the
//! debug shader and sample it per-pixel (for Roughness and Metallic modes).

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct DebugParams {
    /// x = mode (0=normals, 1=roughness, 2=metallic, 3=depth, 4=uv_checker)
    /// y = scalar_roughness, z = scalar_metallic, w = has_mr_texture (0/1)
    pub config: Vec4,
    /// x = depth_near, y = depth_far, z = checker_scale, w = unused
    pub extra: Vec4,
}

impl Default for DebugParams {
    fn default() -> Self {
        Self {
            config: Vec4::new(0.0, 0.5, 0.0, 0.0),
            extra: Vec4::new(0.1, 50.0, 16.0, 0.0),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[derive(Default)]
pub struct ViewportDebugMaterial {
    #[uniform(0)]
    pub params: DebugParams,
    #[texture(1)]
    #[sampler(2)]
    pub mr_texture: Option<Handle<Image>>,
}


impl Material for ViewportDebugMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_viewport/viewport_debug.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_viewport/viewport_debug.wgsl".into()
    }

    // No `specialize`: Bevy's mesh pipeline already builds the vertex layout from
    // the mesh's actual attributes and sets the `VERTEX_NORMALS` / `VERTEX_UVS_A`
    // shader defs to match (for both the main and prepass pipelines, which use
    // *different* attribute locations). The shader gates its inputs on those defs
    // so a mesh missing normals or UV0 still gets a valid pipeline. Overriding
    // the vertex buffers here would force the main-pass locations onto the
    // prepass and crash it (prepass wants normal at @location(3), not @location(1)).
}
