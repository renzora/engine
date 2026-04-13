//! Foliage data types — density maps, type configs, batch markers.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Maximum number of foliage types supported per chunk.
pub const MAX_FOLIAGE_TYPES: usize = 8;

/// Configuration for a single foliage type (e.g. "Short Grass", "Tall Grass", "Wildflowers").
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct FoliageType {
    pub name: String,
    /// Blades per square unit at full density.
    pub density: f32,
    /// Min/max blade height.
    pub height_range: Vec2,
    /// Min/max blade width scale.
    pub width_range: Vec2,
    /// Wind animation strength (0 = no wind, 1 = full).
    pub wind_strength: f32,
    /// Base color (dark, at root).
    pub color_base: LinearRgba,
    /// Tip color (bright, at tip).
    pub color_tip: LinearRgba,
    /// Optional custom shader path override.
    pub shader_path: Option<String>,
    pub enabled: bool,
}

impl Default for FoliageType {
    fn default() -> Self {
        Self {
            name: "Grass".into(),
            density: 32.0,
            height_range: Vec2::new(0.1, 0.4),
            width_range: Vec2::new(0.02, 0.04),
            wind_strength: 1.0,
            color_base: LinearRgba::new(0.12, 0.25, 0.04, 1.0),
            color_tip: LinearRgba::new(0.40, 0.62, 0.18, 1.0),
            shader_path: None,
            enabled: true,
        }
    }
}

/// Project-level foliage type definitions.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct FoliageConfig {
    pub types: Vec<FoliageType>,
}

impl Default for FoliageConfig {
    fn default() -> Self {
        Self {
            types: vec![FoliageType::default()],
        }
    }
}

/// Per-chunk density map for foliage placement.
///
/// Lives alongside `PaintableSurfaceData` on chunk entities. Independent of
/// the terrain splatmap — painting foliage does not affect terrain layers.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct FoliageDensityMap {
    /// Texels per side (e.g. 64 means 64x64 grid).
    pub resolution: u32,
    /// Per-texel weights for each foliage type. `[texel_index][type_index]`.
    /// Values 0.0 (none) to 1.0 (full density).
    pub density_weights: Vec<[f32; MAX_FOLIAGE_TYPES]>,
    /// Set to `true` when weights change; cleared after mesh rebuild.
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
}

impl FoliageDensityMap {
    pub fn new(resolution: u32) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            resolution,
            density_weights: vec![[0.0; MAX_FOLIAGE_TYPES]; count],
            dirty: false,
        }
    }

    /// Sample the density weight for a foliage type at a UV position (0..1, 0..1).
    pub fn sample(&self, uv_x: f32, uv_z: f32, type_index: usize) -> f32 {
        if type_index >= MAX_FOLIAGE_TYPES {
            return 0.0;
        }
        let res = self.resolution;
        let sx = ((uv_x * (res - 1) as f32).round() as u32).min(res - 1);
        let sz = ((uv_z * (res - 1) as f32).round() as u32).min(res - 1);
        let idx = (sz * res + sx) as usize;
        self.density_weights.get(idx).map_or(0.0, |w| w[type_index])
    }
}

/// Marker component for a foliage mesh batch entity (one per chunk per type).
#[derive(Component)]
pub struct FoliageBatch {
    pub foliage_type_index: usize,
    pub chunk_entity: Entity,
}

// ── Foliage painting settings (shared between core + editor) ────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FoliageBrushType {
    #[default]
    Paint,
    Erase,
}

/// Brush settings for foliage painting.
#[derive(Resource, Clone)]
pub struct FoliagePaintSettings {
    pub active_type: usize,
    pub brush_type: FoliageBrushType,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub brush_falloff: f32,
    pub brush_shape: renzora_terrain::data::BrushShape,
    pub falloff_type: renzora_terrain::data::BrushFalloffType,
}

impl Default for FoliagePaintSettings {
    fn default() -> Self {
        Self {
            active_type: 0,
            brush_type: FoliageBrushType::Paint,
            brush_radius: 0.1,
            brush_strength: 0.5,
            brush_falloff: 0.5,
            brush_shape: renzora_terrain::data::BrushShape::Circle,
            falloff_type: renzora_terrain::data::BrushFalloffType::Smooth,
        }
    }
}
