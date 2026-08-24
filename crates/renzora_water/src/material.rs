use bevy::image::Image;
use bevy::pbr::{Material, MaterialPlugin as BevyMaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::component::{WaterSurface, MAX_CASCADES};

/// GPU-side uniform buffer for water shading parameters.
/// Layout must match `WaterUniforms` in `water.wgsl` exactly.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct WaterUniforms {
    /// Deep-water body colour (linear).
    pub water_color: Vec4,
    /// Foam colour (linear).
    pub foam_color: Vec4,
    /// `xyz` = direction the sun points, `w` = normalised intensity. Only the
    /// subsurface-scattering term uses it; everything else comes from Bevy's
    /// own light bindings.
    pub sun_direction: Vec4,
    /// Distance falloffs, all in metres / per-metre:
    /// `(displacement_range, displacement_falloff, normal_falloff, foam_falloff)`.
    ///
    /// The reference hard-codes these (fade displacement past 150 m, flatten
    /// normals at 0.0175/m, fade foam at 0.0075/m) because its ocean mesh is a
    /// fixed +-256 m clipmap. On a larger mesh those constants kill every wave,
    /// normal and whitecap a quarter of the way out, leaving a flat dead plane
    /// for most of the view — so they are scaled by the mesh's actual extent
    /// instead of copied literally.
    pub distance_scales: Vec4,
    /// Per-cascade `(1/tile.x, 1/tile.y, displacement_scale, normal_scale)`.
    pub map_scales: [Vec4; MAX_CASCADES],
    pub num_cascades: u32,
    pub roughness: f32,
    pub normal_strength: f32,
    pub _pad: f32,
}

impl Default for WaterUniforms {
    fn default() -> Self {
        Self {
            water_color: Vec4::new(0.1, 0.15, 0.18, 1.0),
            foam_color: Vec4::new(0.73, 0.67, 0.62, 1.0),
            sun_direction: Vec4::new(0.3, -0.7, 0.4, 1.0),
            distance_scales: Vec4::new(150.0, 0.007, 0.0175, 0.0075),
            map_scales: [Vec4::ZERO; MAX_CASCADES],
            num_cascades: 0,
            roughness: 0.65,
            normal_strength: 1.0,
            _pad: 0.0,
        }
    }
}

/// Custom Bevy Material for water rendering.
///
/// The displacement and normal maps are the cascade texture arrays written by
/// the compute simulation in `sim.rs`; they are sampled in the **vertex** stage
/// as well as the fragment stage, hence the explicit visibility.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub uniforms: WaterUniforms,
    #[texture(1, dimension = "2d_array", visibility(vertex, fragment))]
    #[sampler(2, visibility(vertex, fragment))]
    pub displacements: Option<Handle<Image>>,
    #[texture(3, dimension = "2d_array", visibility(vertex, fragment))]
    #[sampler(4, visibility(vertex, fragment))]
    pub normals: Option<Handle<Image>>,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://renzora_water/water.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_water/water.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // The ocean is opaque (alpha is written as 1.0), but it renders in the
        // transparent phase deliberately. An opaque material is drawn into the
        // depth prepass too, and the prepass would use Bevy's stock vertex
        // shader — the *undisplaced* flat plane — so every wave would be
        // depth-rejected against a mirror-flat prepass. Fixing that properly
        // means shipping a displaced prepass vertex shader; until then the
        // transparent phase is the correct place for it.
        AlphaMode::Blend
    }
}

/// Sync shading parameters from a `WaterSurface` component into `WaterUniforms`.
pub fn sync_uniforms(surface: &WaterSurface, uniforms: &mut WaterUniforms) {
    let c = surface.water_color;
    uniforms.water_color = Vec4::new(c[0], c[1], c[2], 1.0);
    let f = surface.foam_color;
    uniforms.foam_color = Vec4::new(f[0], f[1], f[2], 1.0);
    uniforms.roughness = surface.roughness.clamp(0.0, 1.0);
    uniforms.normal_strength = surface.normal_strength;

    // How much bigger this water is than the +-256 m the reference's constants
    // assume. Capped: past ~3x the geometry is too coarse out there to carry
    // displacement without aliasing as the mesh slides under the camera.
    let half_extent = surface.mesh_half_extent();
    let extent_scale = (half_extent / 256.0).clamp(1.0, 3.0);
    uniforms.distance_scales = Vec4::new(
        150.0 * extent_scale,
        0.007 / extent_scale,
        0.0175 / extent_scale,
        0.0075 / extent_scale,
    );

    let cascades = surface.active_cascades();
    uniforms.num_cascades = cascades.len() as u32;
    uniforms.map_scales = [Vec4::ZERO; MAX_CASCADES];
    for (slot, cascade) in uniforms.map_scales.iter_mut().zip(cascades) {
        let tile = cascade.tile_length.max(Vec2::splat(1e-3));
        *slot = Vec4::new(
            1.0 / tile.x,
            1.0 / tile.y,
            cascade.displacement_scale,
            cascade.normal_scale,
        );
    }
}

/// Plugin that registers the water material type.
pub struct WaterMaterialPlugin;

impl Plugin for WaterMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BevyMaterialPlugin::<WaterMaterial>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `water.wgsl` imports Bevy's mesh/PBR modules, which only exist once
    /// naga_oil has stitched them in at load time — so this test swaps them for
    /// stand-ins and compiles what remains. It proves *this* shader's own code
    /// is valid WGSL (the bicubic filter, the cascade loops, the binding
    /// declarations); it cannot check the Bevy signatures it calls into.
    #[test]
    fn surface_shader_compiles_against_stubs() {
        const STUBS: &str = r#"
struct StubView { world_position: vec4<f32> }
var<private> stub_view: StubView;
struct StubMaterial {
    base_color: vec4<f32>,
    metallic: f32,
    perceptual_roughness: f32,
    reflectance: vec3<f32>,
}
struct PbrInput {
    material: StubMaterial,
    frag_coord: vec4<f32>,
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    N: vec3<f32>,
    V: vec3<f32>,
}
fn stub_pbr_input_new() -> PbrInput {
    var out: PbrInput;
    return out;
}
fn stub_get_world_from_local(index: u32) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(1.0, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, 1.0, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(0.0, 0.0, 0.0, 1.0),
    );
}
fn stub_local_to_world(m: mat4x4<f32>, p: vec4<f32>) -> vec4<f32> { return m * p; }
fn stub_world_to_clip(p: vec3<f32>) -> vec4<f32> { return vec4<f32>(p, 1.0); }
fn stub_calculate_view(p: vec4<f32>, ortho: bool) -> vec3<f32> { return normalize(p.xyz); }
fn stub_apply_pbr_lighting(input: PbrInput) -> vec4<f32> { return input.material.base_color; }
"#;

        let source = include_str!("water.wgsl");
        let body: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("#import"))
            .collect::<Vec<_>>()
            .join("\n")
            .replace(
                "mesh_functions::get_world_from_local",
                "stub_get_world_from_local",
            )
            .replace(
                "mesh_functions::mesh_position_local_to_world",
                "stub_local_to_world",
            )
            .replace("position_world_to_clip", "stub_world_to_clip")
            .replace("view_bindings::view", "stub_view")
            .replace("pbr_functions::calculate_view", "stub_calculate_view")
            .replace("pbr_functions::apply_pbr_lighting", "stub_apply_pbr_lighting")
            .replace("pbr_input_new", "stub_pbr_input_new");
        let combined = format!("{STUBS}\n{body}");

        renzora::wgsl::check(&combined).unwrap_or_else(|err| panic!("water.wgsl: {err}"));
    }

    #[test]
    fn map_scales_invert_tile_length() {
        // The shader multiplies world XZ by these to get cascade UVs; an
        // un-inverted tile length would tile the ocean thousands of times per
        // metre and read as pure noise.
        let surface = WaterSurface::default();
        let mut uniforms = WaterUniforms::default();
        sync_uniforms(&surface, &mut uniforms);

        assert_eq!(uniforms.num_cascades, surface.cascades.len() as u32);
        for (slot, cascade) in uniforms.map_scales.iter().zip(surface.active_cascades()) {
            assert!((slot.x - 1.0 / cascade.tile_length.x).abs() < 1e-6);
            assert!((slot.z - cascade.displacement_scale).abs() < 1e-6);
            assert!((slot.w - cascade.normal_scale).abs() < 1e-6);
        }
    }

    #[test]
    fn extra_cascades_are_ignored() {
        // More cascades than the uniform can hold must clamp, not overflow.
        let mut surface = WaterSurface::default();
        surface.cascades = vec![Default::default(); MAX_CASCADES + 4];
        let mut uniforms = WaterUniforms::default();
        sync_uniforms(&surface, &mut uniforms);
        assert_eq!(uniforms.num_cascades, MAX_CASCADES as u32);
    }
}
