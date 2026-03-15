//! Terrain checkerboard material — procedural world-space checkerboard shader.

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/terrain_checkerboard.wgsl");
        app.add_plugins(MaterialPlugin::<TerrainCheckerboardMaterial>::default());
    }
}

/// Procedural checkerboard material for terrain surfaces.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainCheckerboardMaterial {
    #[uniform(0)]
    pub color_a: LinearRgba,
    #[uniform(0)]
    pub color_b: LinearRgba,
    /// x = scale, y = metallic, z = roughness
    #[uniform(0)]
    pub properties: Vec4,
}

impl Default for TerrainCheckerboardMaterial {
    fn default() -> Self {
        // Dark/light gray checkerboard
        Self {
            color_a: LinearRgba::new(0.32, 0.32, 0.32, 1.0),
            color_b: LinearRgba::new(0.22, 0.22, 0.22, 1.0),
            properties: Vec4::new(0.5, 0.0, 0.8, 0.0),
        }
    }
}

impl Material for TerrainCheckerboardMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_terrain/shaders/terrain_checkerboard.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
