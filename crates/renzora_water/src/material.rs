use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPlugin as BevyMaterialPlugin};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::component::{WaterSurface, GerstnerWave};

/// GPU-side uniform buffer for water parameters.
/// Layout must match `water.wgsl` exactly.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct WaterUniforms {
    // -- Time + new params --
    pub time: f32,
    pub refraction_strength: f32,
    pub max_depth: f32,
    pub caustic_intensity: f32,

    // -- Waves: 6 slots, each packed as (dir.x, dir.y, steepness, wavelength) + (amplitude, 0, 0, 0) --
    pub wave_0: Vec4,
    pub wave_0_amp: Vec4,
    pub wave_1: Vec4,
    pub wave_1_amp: Vec4,
    pub wave_2: Vec4,
    pub wave_2_amp: Vec4,
    pub wave_3: Vec4,
    pub wave_3_amp: Vec4,
    pub wave_4: Vec4,
    pub wave_4_amp: Vec4,
    pub wave_5: Vec4,
    pub wave_5_amp: Vec4,

    // -- Wave count + specular --
    pub wave_count: u32,
    pub specular_power: f32,
    pub wind_speed: f32,
    pub wind_angle: f32,

    // -- Colors --
    pub deep_color: Vec4,
    pub shallow_color: Vec4,
    pub foam_color: Vec4,
    pub sun_direction: Vec4,
    pub absorption_rgb: Vec4,   // (r, g, b, foam_depth)

    // -- Material params --
    pub foam_threshold: f32,
    pub absorption: f32,
    pub roughness: f32,
    pub subsurface_strength: f32,

    // -- Object interactions: 8 slots, each vec4(x, z, radius, submerge) --
    pub obj_0: Vec4,
    pub obj_1: Vec4,
    pub obj_2: Vec4,
    pub obj_3: Vec4,
    pub obj_4: Vec4,
    pub obj_5: Vec4,
    pub obj_6: Vec4,
    pub obj_7: Vec4,
    pub obj_count: u32,
    pub _opad0: f32,
    pub _opad1: f32,
    pub _opad2: f32,
}

impl Default for WaterUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            refraction_strength: 0.03,
            max_depth: 8.0,
            caustic_intensity: 0.2,
            wave_0: Vec4::ZERO,
            wave_0_amp: Vec4::ZERO,
            wave_1: Vec4::ZERO,
            wave_1_amp: Vec4::ZERO,
            wave_2: Vec4::ZERO,
            wave_2_amp: Vec4::ZERO,
            wave_3: Vec4::ZERO,
            wave_3_amp: Vec4::ZERO,
            wave_4: Vec4::ZERO,
            wave_4_amp: Vec4::ZERO,
            wave_5: Vec4::ZERO,
            wave_5_amp: Vec4::ZERO,
            wave_count: 0,
            specular_power: 3000.0,
            wind_speed: 0.3,
            wind_angle: 0.0,
            deep_color: Vec4::new(0.005, 0.02, 0.08, 1.0),
            shallow_color: Vec4::new(0.04, 0.22, 0.28, 1.0),
            foam_color: Vec4::new(0.82, 0.88, 0.92, 1.0),
            sun_direction: Vec4::new(0.3, -0.7, 0.4, 0.0),
            absorption_rgb: Vec4::new(3.0, 1.0, 0.4, 1.0),
            foam_threshold: 0.4,
            absorption: 0.3,
            roughness: 0.15,
            subsurface_strength: 0.3,
            obj_0: Vec4::ZERO,
            obj_1: Vec4::ZERO,
            obj_2: Vec4::ZERO,
            obj_3: Vec4::ZERO,
            obj_4: Vec4::ZERO,
            obj_5: Vec4::ZERO,
            obj_6: Vec4::ZERO,
            obj_7: Vec4::ZERO,
            obj_count: 0,
            _opad0: 0.0,
            _opad1: 0.0,
            _opad2: 0.0,
        }
    }
}

/// Custom Bevy Material for water rendering.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub uniforms: WaterUniforms,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        Self {
            uniforms: WaterUniforms::default(),
        }
    }
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_water/water.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_water/water.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Blend renders after opaques, so the transmission texture
        // contains the full scene for screen-space refraction.
        AlphaMode::Blend
    }
}

/// Pack a `GerstnerWave` into the vec4 pair used by the GPU.
fn pack_wave(wave: &GerstnerWave) -> (Vec4, Vec4) {
    let dir = wave.direction.normalize_or_zero();
    (
        Vec4::new(dir.x, dir.y, wave.steepness, wave.wavelength),
        Vec4::new(wave.amplitude, 0.0, 0.0, 0.0),
    )
}

/// Sync wave data from a `WaterSurface` component into `WaterUniforms`.
pub fn sync_uniforms(surface: &WaterSurface, uniforms: &mut WaterUniforms) {
    let count = surface.waves.len().min(6);
    uniforms.wave_count = count as u32;

    let empty = (Vec4::ZERO, Vec4::ZERO);
    let w = |i: usize| -> (Vec4, Vec4) {
        if i < count { pack_wave(&surface.waves[i]) } else { empty }
    };

    let (p, a) = w(0); uniforms.wave_0 = p; uniforms.wave_0_amp = a;
    let (p, a) = w(1); uniforms.wave_1 = p; uniforms.wave_1_amp = a;
    let (p, a) = w(2); uniforms.wave_2 = p; uniforms.wave_2_amp = a;
    let (p, a) = w(3); uniforms.wave_3 = p; uniforms.wave_3_amp = a;
    let (p, a) = w(4); uniforms.wave_4 = p; uniforms.wave_4_amp = a;
    let (p, a) = w(5); uniforms.wave_5 = p; uniforms.wave_5_amp = a;

    let dc = surface.deep_color;
    uniforms.deep_color = Vec4::new(dc[0], dc[1], dc[2], 1.0);
    let sc = surface.shallow_color;
    uniforms.shallow_color = Vec4::new(sc[0], sc[1], sc[2], 1.0);
    let fc = surface.foam_color;
    uniforms.foam_color = Vec4::new(fc[0], fc[1], fc[2], 1.0);
    uniforms.foam_threshold = surface.foam_threshold;
    uniforms.absorption = surface.absorption;
    uniforms.roughness = surface.roughness;
    uniforms.subsurface_strength = surface.subsurface_strength;
    uniforms.refraction_strength = surface.refraction_strength;
    uniforms.max_depth = surface.max_depth;
    uniforms.caustic_intensity = surface.caustic_intensity;
    uniforms.specular_power = surface.specular_power;
    uniforms.wind_speed = surface.wind_speed;
    uniforms.wind_angle = surface.wind_angle;
    uniforms.absorption_rgb = Vec4::new(
        surface.absorption_r,
        surface.absorption_g,
        surface.absorption_b,
        surface.foam_depth,
    );
}

/// Plugin that registers the water material type.
pub struct WaterMaterialPlugin;

impl Plugin for WaterMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BevyMaterialPlugin::<WaterMaterial>::default());
    }
}
