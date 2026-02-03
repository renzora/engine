//! Terrain data components and resources

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Terrain panel tab selection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TerrainTab {
    #[default]
    Sculpt,
    Paint,
}

/// Brush shape for terrain tools
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrushShape {
    #[default]
    Circle,
    Square,
    Diamond,
}

/// Brush falloff curve type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrushFalloffType {
    #[default]
    Smooth,
    Linear,
    Spherical,
    Tip,
    Flat,
}

/// Flatten mode for the flatten tool
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlattenMode {
    #[default]
    Both,
    Raise,
    Lower,
}

/// Terrain sculpting brush types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum TerrainBrushType {
    /// Raise terrain height
    #[default]
    Raise,
    /// Lower terrain height
    Lower,
    /// Smooth terrain to average nearby heights
    Smooth,
    /// Flatten terrain to a specific height
    Flatten,
    /// Set terrain to exact height
    SetHeight,
    /// Sculpt mode (raise/lower with shift)
    Sculpt,
    /// Erase (reset to default height)
    Erase,
    /// Create ramps
    Ramp,
    /// Thermal erosion
    Erosion,
    /// Hydraulic erosion
    Hydro,
    /// Add procedural noise
    Noise,
    /// Retopologize/aggressive smooth
    Retop,
    /// Toggle terrain visibility
    Visibility,
    /// Blueprint reference plane
    Blueprint,
    /// Mirror terrain
    Mirror,
    /// Select vertices
    Select,
    /// Copy/stamp terrain
    Copy,
}

impl TerrainBrushType {
    pub fn display_name(&self) -> &'static str {
        match self {
            TerrainBrushType::Raise => "Raise",
            TerrainBrushType::Lower => "Lower",
            TerrainBrushType::Smooth => "Smooth",
            TerrainBrushType::Flatten => "Flatten",
            TerrainBrushType::SetHeight => "Set Height",
            TerrainBrushType::Sculpt => "Sculpt",
            TerrainBrushType::Erase => "Erase",
            TerrainBrushType::Ramp => "Ramp",
            TerrainBrushType::Erosion => "Erosion",
            TerrainBrushType::Hydro => "Hydro",
            TerrainBrushType::Noise => "Noise",
            TerrainBrushType::Retop => "Retop",
            TerrainBrushType::Visibility => "Visibility",
            TerrainBrushType::Blueprint => "Blueprint",
            TerrainBrushType::Mirror => "Mirror",
            TerrainBrushType::Select => "Select",
            TerrainBrushType::Copy => "Copy",
        }
    }

    pub fn all() -> &'static [TerrainBrushType] {
        &[
            TerrainBrushType::Raise,
            TerrainBrushType::Lower,
            TerrainBrushType::Smooth,
            TerrainBrushType::Flatten,
            TerrainBrushType::SetHeight,
            TerrainBrushType::Sculpt,
            TerrainBrushType::Erase,
            TerrainBrushType::Ramp,
            TerrainBrushType::Erosion,
            TerrainBrushType::Hydro,
            TerrainBrushType::Noise,
            TerrainBrushType::Retop,
            TerrainBrushType::Visibility,
            TerrainBrushType::Blueprint,
            TerrainBrushType::Mirror,
            TerrainBrushType::Select,
            TerrainBrushType::Copy,
        ]
    }
}

/// Root terrain entity data - stores overall terrain configuration
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainData {
    /// Number of chunks in X direction
    pub chunks_x: u32,
    /// Number of chunks in Z direction
    pub chunks_z: u32,
    /// Size of each chunk in world units
    pub chunk_size: f32,
    /// Number of vertices per chunk edge (e.g., 65 = 64x64 quads per chunk)
    pub chunk_resolution: u32,
    /// Maximum terrain height
    pub max_height: f32,
    /// Minimum terrain height
    pub min_height: f32,
}

impl Default for TerrainData {
    fn default() -> Self {
        Self {
            chunks_x: 4,
            chunks_z: 4,
            chunk_size: 64.0,
            chunk_resolution: 65, // 64x64 quads per chunk
            max_height: 100.0,
            min_height: -10.0,
        }
    }
}

impl TerrainData {
    /// Total terrain width in world units
    pub fn total_width(&self) -> f32 {
        self.chunks_x as f32 * self.chunk_size
    }

    /// Total terrain depth in world units
    pub fn total_depth(&self) -> f32 {
        self.chunks_z as f32 * self.chunk_size
    }

    /// Get the world position of a chunk's corner (min X, min Z)
    pub fn chunk_world_origin(&self, chunk_x: u32, chunk_z: u32) -> Vec3 {
        let half_width = self.total_width() / 2.0;
        let half_depth = self.total_depth() / 2.0;
        Vec3::new(
            chunk_x as f32 * self.chunk_size - half_width,
            0.0,
            chunk_z as f32 * self.chunk_size - half_depth,
        )
    }

    /// Get the spacing between vertices within a chunk
    pub fn vertex_spacing(&self) -> f32 {
        self.chunk_size / (self.chunk_resolution - 1) as f32
    }
}

/// Individual terrain chunk data - stores heightmap for one partition
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainChunkData {
    /// Chunk X index in the terrain grid
    pub chunk_x: u32,
    /// Chunk Z index in the terrain grid
    pub chunk_z: u32,
    /// Heightmap data (normalized 0.0-1.0, row-major)
    /// Size = chunk_resolution * chunk_resolution
    pub heights: Vec<f32>,
    /// Whether the mesh needs to be regenerated
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
}

impl Default for TerrainChunkData {
    fn default() -> Self {
        Self {
            chunk_x: 0,
            chunk_z: 0,
            heights: Vec::new(),
            dirty: true,
        }
    }
}

impl TerrainChunkData {
    /// Create a new chunk with flat terrain at the specified height (0.0-1.0)
    pub fn new(chunk_x: u32, chunk_z: u32, resolution: u32, initial_height: f32) -> Self {
        let size = (resolution * resolution) as usize;
        Self {
            chunk_x,
            chunk_z,
            heights: vec![initial_height; size],
            dirty: true,
        }
    }

    /// Get height at a specific vertex (local coordinates within chunk)
    pub fn get_height(&self, x: u32, z: u32, resolution: u32) -> f32 {
        let idx = (z * resolution + x) as usize;
        self.heights.get(idx).copied().unwrap_or(0.0)
    }

    /// Set height at a specific vertex
    pub fn set_height(&mut self, x: u32, z: u32, resolution: u32, height: f32) {
        let idx = (z * resolution + x) as usize;
        if idx < self.heights.len() {
            self.heights[idx] = height.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    /// Modify height at a specific vertex by a delta
    pub fn modify_height(&mut self, x: u32, z: u32, resolution: u32, delta: f32) {
        let idx = (z * resolution + x) as usize;
        if idx < self.heights.len() {
            self.heights[idx] = (self.heights[idx] + delta).clamp(0.0, 1.0);
            self.dirty = true;
        }
    }
}

/// Marker component linking a chunk to its parent terrain
#[derive(Component)]
pub struct TerrainChunkOf(pub Entity);

/// Resource for terrain tool settings
#[derive(Resource)]
pub struct TerrainSettings {
    /// Currently selected tab (Sculpt/Paint)
    pub tab: TerrainTab,
    /// Currently selected brush type
    pub brush_type: TerrainBrushType,
    /// Brush shape (Circle/Square/Diamond)
    pub brush_shape: BrushShape,
    /// Brush radius in world units
    pub brush_radius: f32,
    /// Brush strength (0.0-1.0)
    pub brush_strength: f32,
    /// Target height for flatten/set height tools (0.0-1.0 normalized)
    pub target_height: f32,
    /// Brush falloff type (0 = linear, 1 = smooth)
    pub falloff: f32,
    /// Brush falloff curve type
    pub falloff_type: BrushFalloffType,
    /// Flatten mode (Both/Raise/Lower)
    pub flatten_mode: FlattenMode,
    /// Whether to use slope-based flattening
    pub use_slope_flatten: bool,
    /// UI section visibility: Tool Settings
    pub section_tool_settings: bool,
    /// UI section visibility: Brush Settings
    pub section_brush_settings: bool,
    /// UI section visibility: Layers
    pub section_layers: bool,
    /// Whether to hide non-terrain meshes while sculpting
    pub hide_non_terrain_meshes: bool,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            tab: TerrainTab::default(),
            brush_type: TerrainBrushType::Raise,
            brush_shape: BrushShape::default(),
            brush_radius: 5.0,
            brush_strength: 0.5,
            target_height: 0.5,
            falloff: 1.0, // Smooth falloff
            falloff_type: BrushFalloffType::default(),
            flatten_mode: FlattenMode::default(),
            use_slope_flatten: false,
            section_tool_settings: true,
            section_brush_settings: true,
            section_layers: false,
            hide_non_terrain_meshes: false,
        }
    }
}

/// State for terrain sculpting operations
#[derive(Resource, Default)]
pub struct TerrainSculptState {
    /// Whether sculpting is currently active (mouse held)
    pub is_sculpting: bool,
    /// Current hover position on terrain (if any)
    pub hover_position: Option<Vec3>,
    /// The terrain entity being sculpted
    pub active_terrain: Option<Entity>,
    /// Height at the point where flatten started (for flatten tool)
    pub flatten_start_height: Option<f32>,
}
