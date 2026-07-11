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
        &[
            Self::Fbm,
            Self::Ridge,
            Self::Billow,
            Self::Warped,
            Self::Hybrid,
        ]
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
        &[
            Self::Add,
            Self::Subtract,
            Self::Replace,
            Self::Max,
            Self::Min,
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
            chunk_resolution: 129,
            // height range 50 with min=-10 → default initial chunk height 0.2 maps
            // to y=0 in chunk-local space (-10 + 0.2*50). Combined with the
            // parent terrain transform at y=0, fresh flat terrain sits on the
            // editor grid plane.
            max_height: 40.0,
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
///
/// Layered model: `base_heights` is the user's authoritative sculpt; `heights`
/// is the composed result (base + Σ enabled edit layers) and is what the mesh
/// and GPU upload read. Sculpt brushes always read/write the base; the
/// composition system recomputes `heights` when anything upstream changes.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainChunkData {
    pub chunk_x: u32,
    pub chunk_z: u32,
    /// User's manual sculpt layer. Sculpt brushes write here.
    #[serde(default, alias = "heights")]
    pub base_heights: Vec<f32>,
    /// Composed buffer: base + all enabled height layer deltas. Read by mesh/GPU.
    /// Rebuilt by the composition system whenever `dirty` is set.
    #[serde(skip, default)]
    #[reflect(ignore)]
    pub heights: Vec<f32>,
    /// Set by writers (sculpt, undo, import); CONSUMED by the composition
    /// system, which turns it into `mesh_stale`. Two flags instead of one so
    /// a writer running at any point in the frame can't lose its edit: with a
    /// single flag, a write landing between composition and the mesh rebuild
    /// got its flag cleared while `heights` stayed stale — sculpting silently
    /// did nothing, depending on ambiguous system order.
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
    /// Set by composition after refreshing `heights`; consumed by the mesh
    /// rebuild system (and read by foliage re-bake, which runs between them).
    #[serde(skip)]
    #[reflect(ignore)]
    pub mesh_stale: bool,
}

impl TerrainChunkData {
    pub fn new(chunk_x: u32, chunk_z: u32, resolution: u32, initial_height: f32) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            chunk_x,
            chunk_z,
            base_heights: vec![initial_height; count],
            heights: vec![initial_height; count],
            dirty: true,
            mesh_stale: false,
        }
    }

    /// Read the composed height (final render value).
    pub fn get_height(&self, x: u32, z: u32, resolution: u32) -> f32 {
        let idx = (z * resolution + x) as usize;
        self.heights.get(idx).copied().unwrap_or(0.0)
    }

    /// Read the base (pre-layer) sculpt height.
    pub fn get_base_height(&self, x: u32, z: u32, resolution: u32) -> f32 {
        let idx = (z * resolution + x) as usize;
        self.base_heights.get(idx).copied().unwrap_or(0.0)
    }

    /// Set the base sculpt height. Marks chunk dirty so composition + mesh rebuild.
    pub fn set_height(&mut self, x: u32, z: u32, resolution: u32, height: f32) {
        let idx = (z * resolution + x) as usize;
        if let Some(h) = self.base_heights.get_mut(idx) {
            *h = height.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    /// Modify the base sculpt height by a delta. Marks chunk dirty.
    pub fn modify_height(&mut self, x: u32, z: u32, resolution: u32, delta: f32) {
        let idx = (z * resolution + x) as usize;
        if let Some(h) = self.base_heights.get_mut(idx) {
            *h = (*h + delta).clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    /// Ensure `heights` buffer matches base in length. Called on load / after
    /// serde deserialization, which skips the composed buffer.
    pub fn ensure_composed_buffer(&mut self) {
        if self.heights.len() != self.base_heights.len() {
            self.heights = self.base_heights.clone();
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
            self.pixels
                .get((yi * self.width + xi) as usize)
                .copied()
                .unwrap_or(0.0)
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
                        if dist < 1.0 {
                            (1.0 - dist * dist).sqrt()
                        } else {
                            0.0
                        }
                    }
                    StampPreset::Cone => (1.0 - dist).max(0.0),
                    StampPreset::Bell => (-dist * dist * 3.0).exp(),
                    StampPreset::Mesa => {
                        if dist < 0.6 {
                            1.0
                        } else if dist < 1.0 {
                            ((1.0 - dist) / 0.4).clamp(0.0, 1.0)
                        } else {
                            0.0
                        }
                    }
                    StampPreset::Ridge => {
                        let ridge = (1.0 - (dz.abs() * 3.0).min(1.0)).max(0.0);
                        let fade = (1.0 - dist).max(0.0);
                        ridge * fade
                    }
                    StampPreset::Crater => {
                        if dist < 0.4 {
                            dist / 0.4 * 0.3
                        } else if dist < 0.7 {
                            0.3 + (dist - 0.4) / 0.3 * 0.7
                        } else if dist < 1.0 {
                            ((1.0 - dist) / 0.3).max(0.0)
                        } else {
                            0.0
                        }
                    }
                    StampPreset::Noise => {
                        if dist < 1.0 {
                            let n = crate::sculpt::fbm(
                                x as f32 * 0.15,
                                z as f32 * 0.15,
                                4,
                                2.0,
                                0.5,
                                42,
                            );
                            n * (1.0 - dist)
                        } else {
                            0.0
                        }
                    }
                };

                pixels[(z * size + x) as usize] = val;
            }
        }

        Self {
            pixels,
            width: size,
            height: size,
            name: kind.display_name().to_string(),
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
        &[
            Self::Dome,
            Self::Cone,
            Self::Bell,
            Self::Mesa,
            Self::Ridge,
            Self::Crater,
            Self::Noise,
        ]
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
        BrushFalloffType::Smooth => (1.0 + (edge_t * std::f32::consts::PI).cos()) * 0.5,
        BrushFalloffType::Linear => 1.0 - edge_t,
        BrushFalloffType::Spherical => (1.0 - edge_t * edge_t).max(0.0).sqrt(),
        BrushFalloffType::Tip => {
            let inv = 1.0 - edge_t;
            inv * inv * inv
        }
        BrushFalloffType::Flat => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn terrain_data_dimension_accessors() {
        let t = TerrainData {
            chunks_x: 3,
            chunks_z: 2,
            chunk_size: 64.0,
            chunk_resolution: 129,
            max_height: 40.0,
            min_height: -10.0,
        };
        assert_eq!(t.total_width(), 192.0);
        assert_eq!(t.total_depth(), 128.0);
        assert_eq!(t.vertex_spacing(), 0.5); // 64 / (129 - 1)
        assert_eq!(t.height_range(), 50.0);
    }

    #[test]
    fn chunk_world_origin_is_centered_on_terrain() {
        let t = TerrainData {
            chunks_x: 4,
            chunks_z: 4,
            chunk_size: 64.0,
            ..TerrainData::default()
        };
        assert_eq!(t.chunk_world_origin(0, 0), Vec3::new(-128.0, 0.0, -128.0));
        // The chunk grid spans symmetrically around the origin.
        assert_eq!(t.chunk_world_origin(2, 2), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(t.chunk_world_origin(3, 1), Vec3::new(64.0, 0.0, -64.0));
    }

    #[test]
    fn chunk_data_new_initializes_both_buffers() {
        let chunk = TerrainChunkData::new(1, 2, 4, 0.3);
        assert_eq!(chunk.chunk_x, 1);
        assert_eq!(chunk.chunk_z, 2);
        assert_eq!(chunk.base_heights, vec![0.3; 16]);
        assert_eq!(chunk.heights, vec![0.3; 16]);
        assert!(chunk.dirty);
    }

    #[test]
    fn chunk_data_height_accessors_use_row_major_index() {
        let mut chunk = TerrainChunkData::new(0, 0, 3, 0.0);
        // (x=2, z=1) → index 1*3+2 = 5 in both buffers.
        chunk.heights[5] = 0.8;
        chunk.base_heights[5] = 0.6;
        assert_eq!(chunk.get_height(2, 1, 3), 0.8);
        assert_eq!(chunk.get_base_height(2, 1, 3), 0.6);
        // Out-of-range reads fall back to 0.0 instead of panicking.
        assert_eq!(chunk.get_height(2, 2, 4), 0.0);
    }

    #[test]
    fn set_height_clamps_and_marks_dirty() {
        let mut chunk = TerrainChunkData::new(0, 0, 3, 0.5);
        chunk.dirty = false;
        chunk.set_height(1, 1, 3, 1.5);
        assert_eq!(chunk.get_base_height(1, 1, 3), 1.0);
        assert!(chunk.dirty);

        chunk.set_height(0, 0, 3, -0.5);
        assert_eq!(chunk.get_base_height(0, 0, 3), 0.0);

        // Out-of-range writes are a no-op and don't dirty the chunk.
        let mut clean = TerrainChunkData::new(0, 0, 3, 0.5);
        clean.dirty = false;
        clean.set_height(5, 5, 3, 0.9);
        assert!(!clean.dirty);
        assert_eq!(clean.base_heights, vec![0.5; 9]);
    }

    #[test]
    fn modify_height_applies_delta_and_clamps() {
        let mut chunk = TerrainChunkData::new(0, 0, 2, 0.5);
        chunk.dirty = false;
        chunk.modify_height(0, 0, 2, 0.2);
        assert!(approx(chunk.get_base_height(0, 0, 2), 0.7));
        assert!(chunk.dirty);
        chunk.modify_height(0, 0, 2, 1.0);
        assert_eq!(chunk.get_base_height(0, 0, 2), 1.0);
        chunk.modify_height(1, 1, 2, -2.0);
        assert_eq!(chunk.get_base_height(1, 1, 2), 0.0);
    }

    #[test]
    fn ensure_composed_buffer_syncs_after_deserialization() {
        // Serde skips `heights`, so loaded chunks start with an empty buffer.
        let mut chunk = TerrainChunkData {
            chunk_x: 0,
            chunk_z: 0,
            base_heights: vec![0.4; 9],
            heights: Vec::new(),
            dirty: false,
        };
        chunk.ensure_composed_buffer();
        assert_eq!(chunk.heights, vec![0.4; 9]);
        assert!(chunk.dirty);

        // Already-synced buffers are left alone.
        chunk.dirty = false;
        chunk.heights[0] = 0.9;
        chunk.ensure_composed_buffer();
        assert_eq!(chunk.heights[0], 0.9);
        assert!(!chunk.dirty);
    }

    #[test]
    fn falloff_is_one_inside_and_zero_at_edge() {
        for ty in [
            BrushFalloffType::Smooth,
            BrushFalloffType::Linear,
            BrushFalloffType::Spherical,
            BrushFalloffType::Tip,
            BrushFalloffType::Flat,
        ] {
            assert_eq!(compute_brush_falloff(0.0, 0.5, ty), 1.0);
            // Inside the inner radius (t <= 1 - falloff) weight stays 1.
            assert_eq!(compute_brush_falloff(0.5, 0.5, ty), 1.0);
            assert_eq!(compute_brush_falloff(1.0, 0.5, ty), 0.0);
            assert_eq!(compute_brush_falloff(1.5, 0.5, ty), 0.0);
        }
    }

    #[test]
    fn falloff_curve_midpoint_values() {
        // falloff = 1.0 → edge_t == t, so midpoints are easy to predict.
        assert!(approx(
            compute_brush_falloff(0.5, 1.0, BrushFalloffType::Smooth),
            0.5
        ));
        assert!(approx(
            compute_brush_falloff(0.5, 1.0, BrushFalloffType::Linear),
            0.5
        ));
        assert!(approx(
            compute_brush_falloff(0.6, 1.0, BrushFalloffType::Spherical),
            0.8
        ));
        assert!(approx(
            compute_brush_falloff(0.5, 1.0, BrushFalloffType::Tip),
            0.125
        ));
        assert_eq!(compute_brush_falloff(0.99, 1.0, BrushFalloffType::Flat), 1.0);
    }

    #[test]
    fn falloff_smooth_is_monotonically_non_increasing() {
        let mut prev = f32::INFINITY;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let w = compute_brush_falloff(t, 0.7, BrushFalloffType::Smooth);
            assert!(w <= prev + 1e-6, "falloff increased at t={t}");
            prev = w;
        }
    }

    #[test]
    fn stamp_sample_bilinear_interpolates() {
        let stamp = StampBrushData {
            pixels: vec![0.0, 1.0, 0.0, 1.0],
            width: 2,
            height: 2,
            name: String::new(),
        };
        assert!(stamp.is_loaded());
        assert_eq!(stamp.sample(0.0, 0.0), 0.0);
        assert_eq!(stamp.sample(1.0, 0.0), 1.0);
        assert_eq!(stamp.sample(1.0, 1.0), 1.0);
        assert!(approx(stamp.sample(0.5, 0.5), 0.5));
        assert!(approx(stamp.sample(0.25, 0.0), 0.25));
    }

    #[test]
    fn stamp_sample_empty_returns_zero() {
        let stamp = StampBrushData::default();
        assert!(!stamp.is_loaded());
        assert_eq!(stamp.sample(0.5, 0.5), 0.0);
    }

    #[test]
    fn stamp_generate_presets_shape_profile() {
        let size = 32u32;
        let center = size / 2; // dx ≈ 0 at this pixel
        let center_idx = (center * size + center) as usize;
        let corner_idx = 0usize;

        let dome = StampBrushData::generate(StampPreset::Dome, size);
        assert_eq!(dome.width, size);
        assert_eq!(dome.height, size);
        assert_eq!(dome.pixels.len(), (size * size) as usize);
        assert!(dome.is_loaded());
        assert!(dome.pixels[center_idx] > 0.95);
        assert_eq!(dome.pixels[corner_idx], 0.0); // corner is outside dist=1

        let cone = StampBrushData::generate(StampPreset::Cone, size);
        assert!(cone.pixels[center_idx] > 0.9);
        // Cone falls off linearly along the center row.
        let row = (center * size) as usize;
        let mut prev = cone.pixels[row + center as usize];
        for x in (center + 1)..size {
            let v = cone.pixels[row + x as usize];
            assert!(v <= prev + 1e-6);
            prev = v;
        }

        let mesa = StampBrushData::generate(StampPreset::Mesa, size);
        // Flat top: every pixel with dist < 0.6 is exactly 1.0.
        assert_eq!(mesa.pixels[center_idx], 1.0);
        assert_eq!(mesa.pixels[(center * size + center + 4) as usize], 1.0);
        assert_eq!(mesa.pixels[corner_idx], 0.0);
    }
}
