//! Foliage data types — density maps, type configs, batch markers.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Maximum number of foliage types supported per chunk.
pub const MAX_FOLIAGE_TYPES: usize = 8;

/// Configuration for a single foliage type (e.g. "Short Grass", "Tall Grass", "Wildflowers").
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct FoliageType {
    pub name: String,
    /// Clumps per square unit at full density — the scatter grid, not the blade
    /// count. Multiply by [`Self::blades_per_clump`] for blades per square unit.
    pub density: f32,
    /// Blades grown from each scatter point. One blade per grid cell leaves
    /// visible ground between cells whatever the density; grass reads as grass
    /// when it comes in tufts, and a tuft is far cheaper than a finer grid
    /// because the density-map sample and the height lookup are shared.
    #[serde(default = "default_blades_per_clump")]
    pub blades_per_clump: u32,
    /// Min/max blade height.
    pub height_range: Vec2,
    /// Min/max blade width, in world units. This is the blade's actual width —
    /// it is not scaled by the blade's height.
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

/// Serde fallback for [`FoliageType::blades_per_clump`], which post-dates the
/// first foliage configs.
fn default_blades_per_clump() -> u32 {
    5
}

impl Default for FoliageType {
    fn default() -> Self {
        Self {
            name: "Grass".into(),
            // 48 clumps x 5 blades = 240 blades per square metre. Counted off a
            // close-up screenshot, the old 32 x 3 landed at ~96/m2 and the
            // ground still read through it; grass only stops looking scattered
            // somewhere north of 200. A chunk-wide blade budget (see
            // `mesh_gen`) is what keeps that affordable when a whole chunk gets
            // painted rather than a patch.
            density: 48.0,
            blades_per_clump: default_blades_per_clump(),
            height_range: Vec2::new(0.1, 0.4),
            // Real-ish blade widths. They read as far wider than the old
            // 0.02–0.04 did, because those were being multiplied by the blade
            // height as well and ended up under a centimetre across.
            width_range: Vec2::new(0.025, 0.05),
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

/// Mask texels per world metre.
///
/// Sized in world units rather than per chunk. A fixed 64² grid gave a 64 m
/// chunk *one texel per metre*, while the brush radius is a fraction of a chunk
/// side that bottoms out at 0.01 — 0.64 m, smaller than a single texel. So the
/// smallest brush painted one texel, and nearest-neighbour sampling rendered
/// that texel as a hard 1 m square of grass around a 1.3 m gizmo. Four texels
/// per metre resolves even the smallest brush to a recognisable disc.
const TEXELS_PER_METRE: f32 = 4.0;
/// Floor on the resolution, so a tiny chunk still gets a usable mask.
const MIN_RESOLUTION: u32 = 64;
/// Ceiling on the resolution. This is a memory limit: a texel carries
/// `MAX_FOLIAGE_TYPES` f32 weights, so 256² is 2 MB per chunk, and the map
/// serializes with the scene.
const MAX_RESOLUTION: u32 = 256;

impl FoliageDensityMap {
    pub fn new(resolution: u32) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            resolution,
            density_weights: vec![[0.0; MAX_FOLIAGE_TYPES]; count],
            dirty: false,
        }
    }

    /// A mask sized for a terrain chunk of `chunk_size` metres, at a fixed
    /// world-space texel density. Prefer this to [`Self::new`] — see
    /// [`TEXELS_PER_METRE`] for why the resolution can't be a constant.
    pub fn for_chunk(chunk_size: f32) -> Self {
        let res = ((chunk_size * TEXELS_PER_METRE).round().max(0.0) as u32)
            .clamp(MIN_RESOLUTION, MAX_RESOLUTION);
        Self::new(res)
    }

    /// Sample the density weight for a foliage type at a UV position (0..1, 0..1).
    ///
    /// Bilinear, not nearest. Nearest makes every texel a hard square of its own
    /// world size, so a patch comes out as a mosaic of blocks that reaches up to
    /// half a texel past wherever the brush actually stopped — which at one texel
    /// per metre was half a metre of grass outside the gizmo.
    pub fn sample(&self, uv_x: f32, uv_z: f32, type_index: usize) -> f32 {
        if type_index >= MAX_FOLIAGE_TYPES || self.resolution == 0 {
            return 0.0;
        }
        let res = self.resolution;
        let max = (res - 1) as f32;
        let fx = (uv_x * max).clamp(0.0, max);
        let fz = (uv_z * max).clamp(0.0, max);
        let x0 = fx.floor() as u32;
        let z0 = fz.floor() as u32;
        let x1 = (x0 + 1).min(res - 1);
        let z1 = (z0 + 1).min(res - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;
        let at = |x: u32, z: u32| -> f32 {
            self.density_weights
                .get((z * res + x) as usize)
                .map_or(0.0, |w| w[type_index])
        };
        let lo = at(x0, z0) * (1.0 - tx) + at(x1, z0) * tx;
        let hi = at(x0, z1) * (1.0 - tx) + at(x1, z1) * tx;
        lo * (1.0 - tz) + hi * tz
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
    pub brush_shape: crate::data::BrushShape,
    pub falloff_type: crate::data::BrushFalloffType,
}

impl Default for FoliagePaintSettings {
    fn default() -> Self {
        Self {
            active_type: 0,
            brush_type: FoliageBrushType::Paint,
            brush_radius: 0.1,
            brush_strength: 0.5,
            brush_falloff: 0.5,
            brush_shape: crate::data::BrushShape::Circle,
            falloff_type: crate::data::BrushFalloffType::Smooth,
        }
    }
}
