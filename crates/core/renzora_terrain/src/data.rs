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
            chunk_resolution: 65,
            max_height: 100.0,
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

    // Terrace
    pub terrace_steps: u32,
    pub terrace_sharpness: f32,
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
            terrace_steps: 8,
            terrace_sharpness: 0.8,
        }
    }
}

/// Whether the terrain tool mode is active.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct TerrainToolState {
    pub active: bool,
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
