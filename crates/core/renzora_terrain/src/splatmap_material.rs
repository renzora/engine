//! Splatmap terrain material — blends up to 8 procedural/textured layers via dual weight textures.

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub struct TerrainSplatmapMaterialPlugin;

impl Plugin for TerrainSplatmapMaterialPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/terrain_splatmap.wgsl");
        app.add_plugins(MaterialPlugin::<TerrainSplatmapMaterial>::default());
    }
}

/// Layer animation types (matches shader switch).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
pub enum LayerAnimationType {
    #[default]
    Solid = 0,
    Grass = 1,
    Water = 2,
    Rock = 3,
    Sand = 4,
    Snow = 5,
    Dirt = 6,
}

impl LayerAnimationType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Grass => "Grass",
            Self::Water => "Water",
            Self::Rock  => "Rock",
            Self::Sand  => "Sand",
            Self::Snow  => "Snow",
            Self::Dirt  => "Dirt",
        }
    }

    pub fn all() -> &'static [LayerAnimationType] {
        &[
            Self::Solid,
            Self::Grass,
            Self::Water,
            Self::Rock,
            Self::Sand,
            Self::Snow,
            Self::Dirt,
        ]
    }
}

/// Per-layer colors packed for 8 layers (two blocks of 4).
/// Each vec4: rgb = base color, a = uv_scale.
#[derive(Clone, Debug, ShaderType)]
pub struct LayerColorBlock {
    pub v0: Vec4,
    pub v1: Vec4,
    pub v2: Vec4,
    pub v3: Vec4,
}

impl Default for LayerColorBlock {
    fn default() -> Self {
        Self {
            v0: Vec4::new(0.25, 0.50, 0.15, 0.1),
            v1: Vec4::new(0.50, 0.45, 0.35, 0.1),
            v2: Vec4::new(0.10, 0.25, 0.40, 0.1),
            v3: Vec4::new(0.40, 0.38, 0.35, 0.1),
        }
    }
}

/// Per-layer properties packed for 4 layers.
/// Each vec4: x = metallic, y = roughness, z = anim_type, w = anim_speed.
#[derive(Clone, Debug, ShaderType)]
pub struct LayerPropsBlock {
    pub v0: Vec4,
    pub v1: Vec4,
    pub v2: Vec4,
    pub v3: Vec4,
}

impl Default for LayerPropsBlock {
    fn default() -> Self {
        Self {
            v0: Vec4::new(0.0, 0.8, 1.0, 1.0),
            v1: Vec4::new(0.0, 0.9, 6.0, 0.0),
            v2: Vec4::new(0.1, 0.2, 2.0, 1.0),
            v3: Vec4::new(0.0, 0.9, 3.0, 0.0),
        }
    }
}

/// Uniform holding the active layer count.
#[derive(Clone, Debug, ShaderType)]
pub struct TerrainLayerInfo {
    pub layer_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

impl Default for TerrainLayerInfo {
    fn default() -> Self {
        Self {
            layer_count: 4,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        }
    }
}

/// PBR-lit splatmap material that blends up to 8 procedural/textured layers.
///
/// Bindings:
///   0: layer_colors_a (layers 0-3 color+uv_scale)
///   1: layer_props_a  (layers 0-3 metallic/roughness/anim_type/speed)
///   2: splatmap_a     (layers 0-3 weights RGBA8)
///   3: splat_sampler
///   4: layer_colors_b (layers 4-7)
///   5: layer_props_b  (layers 4-7)
///   6: splatmap_b     (layers 4-7 weights RGBA8)
///   7: layer_info     (layer count)
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainSplatmapMaterial {
    #[uniform(0)]
    pub layer_colors_a: LayerColorBlock,
    #[uniform(1)]
    pub layer_props_a: LayerPropsBlock,
    #[texture(2)]
    #[sampler(3)]
    pub splatmap_a: Handle<Image>,
    #[uniform(4)]
    pub layer_colors_b: LayerColorBlock,
    #[uniform(5)]
    pub layer_props_b: LayerPropsBlock,
    #[texture(6, sample_type = "float", dimension = "2d")]
    pub splatmap_b: Handle<Image>,
    #[uniform(7)]
    pub layer_info: TerrainLayerInfo,
}

impl Material for TerrainSplatmapMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_terrain/shaders/terrain_splatmap.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
