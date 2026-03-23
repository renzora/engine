//! Terrain Material - Custom material that uses generated WGSL shaders
//!
//! This provides a proper runtime shader for terrain instead of baked textures.

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use std::path::PathBuf;

use crate::blueprint::{compile_material_blueprint, generate_wgsl_code};
use crate::blueprint::serialization::BlueprintFile;
use crate::project::CurrentProject;

/// Plugin for terrain material support
pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
            .init_resource::<TerrainMaterialState>()
            .add_systems(Startup, setup_terrain_material);
    }
}

/// State for terrain material shader generation
#[derive(Resource, Default)]
pub struct TerrainMaterialState {
    /// Whether the shader has been generated
    pub shader_generated: bool,
    /// Path to the generated shader
    pub shader_path: Option<PathBuf>,
    /// Handle to the terrain material
    pub material_handle: Option<Handle<TerrainMaterial>>,
}

/// Custom terrain material that uses the generated procedural shader
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct TerrainMaterial {
    /// Base color tint (multiplied with shader output)
    #[uniform(0)]
    pub base_color: LinearRgba,

    /// Material properties: metallic, roughness, ao, unused
    #[uniform(1)]
    pub properties: Vec4,
}

impl Material for TerrainMaterial {
    fn fragment_shader() -> ShaderRef {
        // Use the generated terrain shader
        ShaderRef::Path("shaders/generated/terrain_material.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

/// System to generate the terrain material shader from blueprint
fn setup_terrain_material(
    mut state: ResMut<TerrainMaterialState>,
    current_project: Option<Res<CurrentProject>>,
) {
    // Try to find and compile the checkerboard material blueprint
    let blueprint_paths = [
        current_project.as_ref().map(|p| p.path.join("assets/materials/checkerboard_default.material_bp")),
        Some(PathBuf::from("assets/materials/checkerboard_default.material_bp")),
    ];

    for path_option in blueprint_paths.iter().flatten() {
        if path_option.exists() {
            if let Ok(blueprint_file) = BlueprintFile::load(path_option) {
                // Generate WGSL code
                let result = generate_wgsl_code(&blueprint_file.graph);

                if result.errors.is_empty() {
                    // Ensure the output directory exists
                    let shader_dir = PathBuf::from("assets/shaders/generated");
                    if std::fs::create_dir_all(&shader_dir).is_ok() {
                        let shader_path = shader_dir.join("terrain_material.wgsl");

                        // Write the shader
                        if std::fs::write(&shader_path, &result.fragment_shader).is_ok() {
                            info!("Generated terrain shader: {:?}", shader_path);
                            state.shader_generated = true;
                            state.shader_path = Some(shader_path);
                            return;
                        }
                    }
                }

                for err in &result.errors {
                    warn!("Terrain shader error: {}", err);
                }
            }
        }
    }

    // Fallback: write a default procedural checkerboard shader
    info!("Using fallback terrain shader");
    write_fallback_terrain_shader(&mut state);
}

/// Write a fallback procedural checkerboard shader
fn write_fallback_terrain_shader(state: &mut TerrainMaterialState) {
    let shader_code = r#"
// Fallback Terrain Material - Procedural Checkerboard
// This runs per-pixel for infinite resolution tiling

#import bevy_pbr::{
    pbr_functions::pbr,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

struct TerrainMaterialUniform {
    base_color: vec4<f32>,
    properties: vec4<f32>, // metallic, roughness, ao, unused
}

@group(2) @binding(0) var<uniform> material: TerrainMaterialUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Use world position for tiling (not UVs)
    let world_pos = in.world_position.xz;

    // Checkerboard scale (squares per world unit)
    let scale = 0.5;

    // Calculate checkerboard pattern
    let checker = floor(world_pos.x * scale) + floor(world_pos.y * scale);
    let checker_value = fract(checker * 0.5) * 2.0;

    // Two-tone colors
    let color_a = vec3<f32>(0.85, 0.85, 0.85); // Light gray
    let color_b = vec3<f32>(0.65, 0.65, 0.65); // Darker gray
    let base_color = mix(color_a, color_b, checker_value);

    // PBR setup
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = vec4<f32>(base_color * material.base_color.rgb, 1.0);
    pbr_input.material.metallic = material.properties.x;
    pbr_input.material.perceptual_roughness = material.properties.y;
    pbr_input.occlusion = vec3<f32>(material.properties.z);
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.world_position = vec4<f32>(in.world_position, 1.0);
    pbr_input.frag_coord = in.position;

    var color = pbr(pbr_input);
    color.a = 1.0;
    return color;
}
"#;

    let shader_dir = PathBuf::from("assets/shaders/generated");
    if std::fs::create_dir_all(&shader_dir).is_ok() {
        let shader_path = shader_dir.join("terrain_material.wgsl");
        if std::fs::write(&shader_path, shader_code).is_ok() {
            info!("Generated fallback terrain shader: {:?}", shader_path);
            state.shader_generated = true;
            state.shader_path = Some(shader_path);
        }
    }
}
