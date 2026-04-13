//! Custom material used by viewport visualization modes.
//!
//! One [`ViewportDebugMaterial`] is created per source [`StandardMaterial`]
//! so that we can carry the original metallic-roughness texture into the
//! debug shader and sample it per-pixel (for Roughness and Metallic modes).

use bevy::prelude::*;
use bevy::pbr::Material;
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
pub struct ViewportDebugMaterial {
    #[uniform(0)]
    pub params: DebugParams,
    #[texture(1)]
    #[sampler(2)]
    pub mr_texture: Option<Handle<Image>>,
}

impl Default for ViewportDebugMaterial {
    fn default() -> Self {
        Self {
            params: DebugParams::default(),
            mr_texture: None,
        }
    }
}

impl Material for ViewportDebugMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_viewport/viewport_debug.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_viewport/viewport_debug.wgsl".into()
    }
}
