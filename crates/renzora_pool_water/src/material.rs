use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// GPU uniform buffer for pool water rendering.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct PoolWaterUniforms {
    /// Directional light direction
    pub light_direction: Vec4,
    /// Deep water absorption color (Beer's law)
    pub deep_color: Vec4,
    /// Shallow water tint
    pub shallow_color: Vec4,
    /// Index of refraction
    pub ior: f32,
    /// Minimum Fresnel reflectance
    pub fresnel_min: f32,
    /// Caustic brightness
    pub caustic_intensity: f32,
    /// Current time
    pub time: f32,
    /// Height scale — maps simulation values to world units
    pub height_scale: f32,
    /// Specular power for sun highlight
    pub specular_power: f32,
    /// Refraction distortion strength
    pub refraction_strength: f32,
    /// Maximum absorption depth (world units)
    pub max_depth: f32,
    /// Absorption coefficients (r, g, b, foam_depth)
    pub absorption: Vec4,
    /// Shoreline foam color
    pub foam_color: Vec4,
}

impl Default for PoolWaterUniforms {
    fn default() -> Self {
        Self {
            light_direction: Vec4::new(0.3, -0.7, 0.4, 0.0),
            deep_color: Vec4::new(0.005, 0.02, 0.08, 1.0),
            shallow_color: Vec4::new(0.04, 0.22, 0.28, 1.0),
            ior: 1.333,
            fresnel_min: 0.02,
            caustic_intensity: 0.25,
            time: 0.0,
            height_scale: 0.3,
            specular_power: 5000.0,
            refraction_strength: 0.03,
            max_depth: 5.0,
            // Red absorbed fastest, green moderate, blue least (like real water)
            absorption: Vec4::new(3.0, 1.0, 0.4, 1.0),
            foam_color: Vec4::new(0.9, 0.92, 0.95, 1.0),
        }
    }
}

/// Custom Bevy Material for interactive pool water with screen-space refraction.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PoolWaterMaterial {
    #[uniform(0)]
    pub uniforms: PoolWaterUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub heightfield: Option<Handle<Image>>,
}

impl Default for PoolWaterMaterial {
    fn default() -> Self {
        Self {
            uniforms: PoolWaterUniforms::default(),
            heightfield: None,
        }
    }
}

impl Material for PoolWaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_pool_water/pool_water.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_pool_water/pool_water.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Blend renders after opaques, so the transmission texture
        // contains the full scene for screen-space refraction.
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        0.0
    }
}
