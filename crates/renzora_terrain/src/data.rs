//! Terrain data types — heightmap, chunks, brush settings.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TerrainTab {
    #[default]
    Sculpt,
    Paint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum BrushShape {
    #[default]
    Circle,
    Square,
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum BrushFalloffType {
    #[default]
    Smooth,
    Linear,
    Spherical,
    Tip,
    Flat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum FlattenMode {
    #[default]
    Both,
    Raise,
    Lower,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum NoiseMode {
    #[default]
    Fbm,
    Ridge,
    Billow,
    Warped,
    Hybrid,
}

impl NoiseMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Fbm => "FBM",
            Self::Ridge => "Ridge",
            Self::Billow => "Billow",
            Self::Warped => "Warped",
            Self::Hybrid => "Hybrid",
        }
    }

    pub fn all() -> &'static [NoiseMode] {
        &[Self::Fbm, Self::Ridge, Self::Billow, Self::Warped, Self::Hybrid]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum TerrainBrushType {
    #[default]
    Raise,
    Lower,
    Smooth,
    Flatten,
    SetHeight,
    Sculpt,
    Erase,
    Ramp,
    Erosion,
    Hydro,
    Noise,
    Retop,
    Terrace,
    Pinch,
    Relax,
    Cliff,
    Stamp,
}

/// How the stamp image blends with existing terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum StampBlendMode {
    /// Add stamp heights on top of existing terrain.
    #[default]
    Add,
    /// Subtract stamp heights from existing terrain.
    Subtract,
    /// Replace terrain with stamp heights directly.
    Replace,
    /// Use the maximum of stamp and existing terrain.
    Max,
    /// Use the minimum of stamp and existing terrain.
    Min,
}

impl TerrainBrushType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Raise => "Raise",
            Self::Lower => "Lower",
            Self::Smooth => "Smooth",
            Self::Flatten => "Flatten",
            Self::SetHeight => "Set Height",
            Self::Sculpt => "Sculpt",
            Self::Erase => "Erase",
            Self::Ramp => "Ramp",
            Self::Erosion => "Erosion",
            Self::Hydro => "Hydraulic",
            Self::Noise => "Noise",
            Self::Retop => "Retop",
            Self::Terrace => "Terrace",
            Self::Pinch => "Pinch",
            Self::Relax => "Relax",
            Self::Cliff => "Cliff",
            Self::Stamp => "Stamp",
        }
    }

    pub fn all() -> &'static [TerrainBrushType] {
        &[
            Self::Sculpt,
            Self::Raise,
            Self::Lower,
            Self::Smooth,
            Self::Flatten,
            Self::SetHeight,
            Self::Erase,
            Self::Noise,
            Self::Stamp,
            Self::Erosion,
            Self::Hydro,
            Self::Terrace,
            Self::Pinch,
            Self::Relax,
            Self::Cliff,
            Self::Retop,
            Self::Ramp,
        ]
    }
}

impl StampBlendMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Replace => "Replace",
            Self::Max => "Max",
            Self::Min => "Min",
        }
    }

    pub fn all() -> &'static [StampBlendMode] {
        &[Self::Add, Self::Subtract, Self::Replace, Self::Max, Self::Min]
    }
}

// ── Components ──────────────────────────────────────────────────────────────

/// Root terrain entity — defines the chunk grid and height range.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainData {
    pub chunks_x: u32,
    pub chunks_z: u32,
    pub chunk_size: f32,
    pub chunk_resolution: u32,
    pub max_height: f32,
    pub min_height: f32,
}

impl Default for TerrainData {
    fn default() -> Self {
        Self {
            chunks_x: 4,
            chunks_z: 4,
            chunk_size: 64.0,
            chunk_resolution: 129,
            max_height: 50.0,
            min_height: -10.0,
        }
    }
}

impl TerrainData {
    pub fn total_width(&self) -> f32 {
        self.chunks_x as f32 * self.chunk_size
    }

    pub fn total_depth(&self) -> f32 {
        self.chunks_z as f32 * self.chunk_size
    }

    pub fn chunk_world_origin(&self, chunk_x: u32, chunk_z: u32) -> Vec3 {
        let half_w = self.total_width() / 2.0;
        let half_d = self.total_depth() / 2.0;
        Vec3::new(
            chunk_x as f32 * self.chunk_size - half_w,
            0.0,
            chunk_z as f32 * self.chunk_size - half_d,
        )
    }

    pub fn vertex_spacing(&self) -> f32 {
        self.chunk_size / (self.chunk_resolution - 1) as f32
    }

    pub fn height_range(&self) -> f32 {
        self.max_height - self.min_height
    }
}

/// Per-chunk heightmap data. Heights are normalized [0, 1].
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainChunkData {
    pub chunk_x: u32,
    pub chunk_z: u32,
    pub heights: Vec<f32>,
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
}

impl TerrainChunkData {
    pub fn new(chunk_x: u32, chunk_z: u32, resolution: u32, initial_height: f32) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            chunk_x,
            chunk_z,
            heights: vec![initial_height; count],
            dirty: true,
        }
    }

    pub fn get_height(&self, x: u32, z: u32, resolution: u32) -> f32 {
        let idx = (z * resolution + x) as usize;
        self.heights.get(idx).copied().unwrap_or(0.0)
    }

    pub fn set_height(&mut self, x: u32, z: u32, resolution: u32, height: f32) {
        let idx = (z * resolution + x) as usize;
        if let Some(h) = self.heights.get_mut(idx) {
            *h = height.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    pub fn modify_height(&mut self, x: u32, z: u32, resolution: u32, delta: f32) {
        let idx = (z * resolution + x) as usize;
        if let Some(h) = self.heights.get_mut(idx) {
            *h = (*h + delta).clamp(0.0, 1.0);
            self.dirty = true;
        }
    }
}

/// Links a chunk entity back to its parent terrain entity.
#[derive(Component)]
pub struct TerrainChunkOf(pub Entity);

// ── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TerrainSettings {
    pub tab: TerrainTab,
    pub brush_type: TerrainBrushType,
    pub brush_shape: BrushShape,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub target_height: f32,
    pub falloff: f32,
    pub falloff_type: BrushFalloffType,
    pub flatten_mode: FlattenMode,

    // Noise
    pub noise_scale: f32,
    pub noise_octaves: u32,
    pub noise_lacunarity: f32,
    pub noise_persistence: f32,
    pub noise_seed: u32,
    pub noise_mode: NoiseMode,
    pub warp_strength: f32,

    // Terrace
    pub terrace_steps: u32,
    pub terrace_sharpness: f32,

    // Stamp
    pub stamp_blend_mode: StampBlendMode,
    pub stamp_rotation: f32,
    pub stamp_height_scale: f32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            tab: TerrainTab::Sculpt,
            brush_type: TerrainBrushType::Sculpt,
            brush_shape: BrushShape::Circle,
            brush_radius: 20.0,
            brush_strength: 0.5,
            target_height: 0.5,
            falloff: 0.7,
            falloff_type: BrushFalloffType::Smooth,
            flatten_mode: FlattenMode::Both,
            noise_scale: 30.0,
            noise_octaves: 5,
            noise_lacunarity: 2.0,
            noise_persistence: 0.5,
            noise_seed: 42,
            noise_mode: NoiseMode::Fbm,
            warp_strength: 0.5,
            terrace_steps: 8,
            terrace_sharpness: 0.8,
            stamp_blend_mode: StampBlendMode::Add,
            stamp_rotation: 0.0,
            stamp_height_scale: 1.0,
        }
    }
}

/// Whether the terrain tool mode is active.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct TerrainToolState {
    pub active: bool,
}

/// Loaded stamp image data for the Stamp brush.
#[derive(Resource, Default)]
pub struct StampBrushData {
    /// Grayscale pixel data, normalized [0, 1]. Row-major.
    pub pixels: Vec<f32>,
    /// Width of the stamp image.
    pub width: u32,
    /// Height of the stamp image.
    pub height: u32,
    /// Display name of the loaded file.
    pub name: String,
}

impl StampBrushData {
    /// Sample the stamp at UV coordinates (0..1, 0..1) with bilinear interpolation.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        if self.pixels.is_empty() || self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let x = u * (self.width - 1) as f32;
        let y = v * (self.height - 1) as f32;
        let x0 = (x.floor() as u32).min(self.width - 1);
        let y0 = (y.floor() as u32).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x.fract();
        let ty = y.fract();
        let get = |xi: u32, yi: u32| -> f32 {
            self.pixels.get((yi * self.width + xi) as usize).copied().unwrap_or(0.0)
        };
        let h0 = get(x0, y0) * (1.0 - tx) + get(x1, y0) * tx;
        let h1 = get(x0, y1) * (1.0 - tx) + get(x1, y1) * tx;
        h0 * (1.0 - ty) + h1 * ty
    }

    pub fn is_loaded(&self) -> bool {
        !self.pixels.is_empty()
    }

    /// Load from a PNG file (reuses heightmap_import's PNG loader).
    pub fn load_from_png(data: &[u8]) -> Result<(u32, u32, Vec<f32>), String> {
        crate::heightmap_import::load_png_public(data)
    }

    /// Generate a procedural stamp. `kind` selects the preset.
    pub fn generate(kind: StampPreset, size: u32) -> Self {
        let mut pixels = vec![0.0f32; (size * size) as usize];
        let center = size as f32 / 2.0;
        let radius = center;

        for z in 0..size {
            for x in 0..size {
                let dx = (x as f32 + 0.5 - center) / radius;
                let dz = (z as f32 + 0.5 - center) / radius;
                let dist = (dx * dx + dz * dz).sqrt();

                let val = match kind {
                    StampPreset::Dome => {
                        if dist < 1.0 { (1.0 - dist * dist).sqrt() } else { 0.0 }
                    }
                    StampPreset::Cone => {
                        (1.0 - dist).max(0.0)
                    }
                    StampPreset::Bell => {
                        (-dist * dist * 3.0).exp()
                    }
                    StampPreset::Mesa => {
                        if dist < 0.6 { 1.0 }
                        else if dist < 1.0 { ((1.0 - dist) / 0.4).clamp(0.0, 1.0) }
                        else { 0.0 }
                    }
                    StampPreset::Ridge => {
                        let ridge = (1.0 - (dz.abs() * 3.0).min(1.0)).max(0.0);
                        let fade = (1.0 - dist).max(0.0);
                        ridge * fade
                    }
                    StampPreset::Crater => {
                        if dist < 0.4 { dist / 0.4 * 0.3 }
                        else if dist < 0.7 { 0.3 + (dist - 0.4) / 0.3 * 0.7 }
                        else if dist < 1.0 { ((1.0 - dist) / 0.3).max(0.0) }
                        else { 0.0 }
                    }
                    StampPreset::Noise => {
                        if dist < 1.0 {
                            let n = crate::sculpt::fbm(
                                x as f32 * 0.15,
                                z as f32 * 0.15,
                                4, 2.0, 0.5, 42,
                            );
                            n * (1.0 - dist)
                        } else { 0.0 }
                    }
                };

                pixels[(z * size + x) as usize] = val;
            }
        }

        Self {
            pixels,
            width: size,
            height: size,
            name: format!("{}", kind.display_name()),
        }
    }
}

/// Built-in procedural stamp presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampPreset {
    Dome,
    Cone,
    Bell,
    Mesa,
    Ridge,
    Crater,
    Noise,
}

impl StampPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Dome => "Dome",
            Self::Cone => "Cone",
            Self::Bell => "Bell",
            Self::Mesa => "Mesa",
            Self::Ridge => "Ridge",
            Self::Crater => "Crater",
            Self::Noise => "Noise",
        }
    }

    pub fn all() -> &'static [StampPreset] {
        &[Self::Dome, Self::Cone, Self::Bell, Self::Mesa, Self::Ridge, Self::Crater, Self::Noise]
    }
}

#[derive(Resource, Default)]
pub struct TerrainSculptState {
    pub is_sculpting: bool,
    pub hover_position: Option<Vec3>,
    pub active_terrain: Option<Entity>,
    pub flatten_start_height: Option<f32>,
    pub brush_visible: bool,
}

// ── Utility ─────────────────────────────────────────────────────────────────

/// Compute brush falloff weight for a normalized distance `t` (0=center, 1=edge).
pub fn compute_brush_falloff(t: f32, falloff: f32, falloff_type: BrushFalloffType) -> f32 {
    if t >= 1.0 {
        return 0.0;
    }
    let inner_t = 1.0 - falloff;
    if t <= inner_t {
        return 1.0;
    }
    let edge_t = (t - inner_t) / falloff.max(0.001);
    match falloff_type {
        BrushFalloffType::Smooth => {
            (1.0 + (edge_t * std::f32::consts::PI).cos()) * 0.5
        }
        BrushFalloffType::Linear => {
            1.0 - edge_t
        }
        BrushFalloffType::Spherical => {
            (1.0 - edge_t * edge_t).max(0.0).sqrt()
        }
        BrushFalloffType::Tip => {
            let inv = 1.0 - edge_t;
            inv * inv * inv
        }
        BrushFalloffType::Flat => 1.0,
    }
}
