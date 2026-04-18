//! Grass material — custom Bevy Material with wind-animated vertex shader.

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// GPU-side uniform buffer for grass parameters.
/// Layout must match `grass.wgsl` exactly.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct GrassUniforms {
    pub time: f32,
    pub wind_strength: f32,
    pub wind_direction: Vec2,
    pub color_base: Vec4,
    pub color_tip: Vec4,
    pub chunk_world_x: f32,
    pub chunk_world_z: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Default for GrassUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            wind_strength: 1.0,
            wind_direction: Vec2::new(0.7, 0.3),
            color_base: Vec4::new(0.12, 0.25, 0.04, 1.0),
            color_tip: Vec4::new(0.40, 0.62, 0.18, 1.0),
            chunk_world_x: 0.0,
            chunk_world_z: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GrassMaterial {
    #[uniform(0)]
    pub uniforms: GrassUniforms,
}

impl Default for GrassMaterial {
    fn default() -> Self {
        Self {
            uniforms: GrassUniforms::default(),
        }
    }
}

impl Material for GrassMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_foliage/grass.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_foliage/grass.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
