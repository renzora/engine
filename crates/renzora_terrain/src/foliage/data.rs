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
    /// How strongly this foliage layer responds to the world wind
    /// (`renzora::WindState`), 0 = rigid, 1 = full. It is a multiplier, not a
    /// wind: turning the world wind down calms every layer regardless.
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
    /// Per-texel blade-height multiplier, 1.0 being whatever the foliage type's
    /// own `height_range` asks for. Painted with the Grow and Trim brushes.
    ///
    /// One channel for the whole chunk rather than one per foliage type. Height
    /// is a property of the *ground* — a sheltered hollow grows everything in it
    /// taller, a trodden path keeps everything short — and a per-type copy would
    /// carry [`MAX_FOLIAGE_TYPES`] f32s per texel, doubling a map that already
    /// serializes into the scene at up to 2 MB a chunk.
    ///
    /// Empty means "nothing painted, neutral everywhere". That is both the
    /// default and what every scene saved before this field existed
    /// deserializes to, so the channel costs nothing at all until someone picks
    /// up the brush. Read it through [`Self::height_at`], which knows that —
    /// indexing it directly will be wrong on most chunks.
    ///
    /// **Both** default attributes are load-bearing, and for two different
    /// paths. `serde(default)` covers the config/serde side; `reflect(default)`
    /// covers scene loading, which reconstructs the component through
    /// `FromReflect` — and `FromReflect` on a struct fails outright when a field
    /// is absent from the serialized data unless the field says otherwise. Ship
    /// this without the reflect attribute and every scene saved before the
    /// height channel existed dies on load with "couldn't create an instance of
    /// `FoliageDensityMap`".
    #[serde(default)]
    #[reflect(default)]
    pub height_scale: Vec<f32>,
    /// Set to `true` when weights change; cleared after mesh rebuild.
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
}

/// Floor on the painted blade-height multiplier.
///
/// Not zero: grass trimmed away to nothing is just a gap in the field, and the
/// density mask is the tool for cutting one. This is "mown lawn", not "bare
/// earth".
pub const MIN_HEIGHT_SCALE: f32 = 0.25;
/// Ceiling on the painted blade-height multiplier.
pub const MAX_HEIGHT_SCALE: f32 = 3.0;
/// The multiplier an unpainted texel has, and the value both height brushes
/// pass through on their way between the two bounds.
pub const NEUTRAL_HEIGHT_SCALE: f32 = 1.0;

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
            // Deliberately unallocated — see the field docs.
            height_scale: Vec::new(),
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
        self.bilinear(uv_x, uv_z, |idx| {
            self.density_weights.get(idx).map_or(0.0, |w| w[type_index])
        })
        .unwrap_or(0.0)
    }

    /// Bilinear blade-height multiplier at a UV position.
    ///
    /// Returns [`NEUTRAL_HEIGHT_SCALE`] wherever the channel is unallocated,
    /// which is every chunk nobody has run a height brush over — including every
    /// scene saved before the channel existed.
    ///
    /// Bilinear for the same reason [`Self::sample`] is: nearest would turn each
    /// texel into a hard square of its own world size, and a step change in
    /// blade height reads as a visible tile edge running through the field.
    pub fn height_at(&self, uv_x: f32, uv_z: f32) -> f32 {
        if self.resolution == 0 || self.height_scale.is_empty() {
            return NEUTRAL_HEIGHT_SCALE;
        }
        self.bilinear(uv_x, uv_z, |idx| {
            self.height_scale
                .get(idx)
                .copied()
                .unwrap_or(NEUTRAL_HEIGHT_SCALE)
        })
        .unwrap_or(NEUTRAL_HEIGHT_SCALE)
    }

    /// Allocate the height channel at neutral if it isn't already sized.
    ///
    /// Called by the brush rather than by the constructor, so a chunk nobody has
    /// painted heights on never carries the memory or serializes the field.
    pub fn ensure_height_scale(&mut self) {
        let count = (self.resolution * self.resolution) as usize;
        if self.height_scale.len() != count {
            self.height_scale = vec![NEUTRAL_HEIGHT_SCALE; count];
        }
    }

    /// Bilinear tap shared by [`Self::sample`] and [`Self::height_at`], so the
    /// two channels can never disagree about where a UV lands. `at` is handed a
    /// flat texel index. `None` when the map has no texels.
    fn bilinear(&self, uv_x: f32, uv_z: f32, at: impl Fn(usize) -> f32) -> Option<f32> {
        let res = self.resolution;
        if res == 0 {
            return None;
        }
        let max = (res - 1) as f32;
        let fx = (uv_x * max).clamp(0.0, max);
        let fz = (uv_z * max).clamp(0.0, max);
        let x0 = fx.floor() as u32;
        let z0 = fz.floor() as u32;
        let x1 = (x0 + 1).min(res - 1);
        let z1 = (z0 + 1).min(res - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;
        let tap = |x: u32, z: u32| at((z * res + x) as usize);
        let lo = tap(x0, z0) * (1.0 - tx) + tap(x1, z0) * tx;
        let hi = tap(x0, z1) * (1.0 - tx) + tap(x1, z1) * tx;
        Some(lo * (1.0 - tz) + hi * tz)
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
    /// Raise the blade-height multiplier toward [`MAX_HEIGHT_SCALE`].
    Grow,
    /// Lower it toward [`MIN_HEIGHT_SCALE`].
    Trim,
}

impl FoliageBrushType {
    /// Whether this brush writes the height channel rather than the density one.
    ///
    /// The two channels are painted by the same stroke code and differ only in
    /// what a texel's `effect` is applied to, so this is the single place the
    /// distinction is drawn.
    pub fn paints_height(self) -> bool {
        matches!(self, Self::Grow | Self::Trim)
    }

    /// The value this brush drags a texel toward. Every brush here is the same
    /// shape — an exponential approach to a target, at the falloff-weighted rate
    /// — so the mode only has to name its target.
    ///
    /// The density modes have a fixed one; the height modes take
    /// [`FoliagePaintSettings::brush_height`], the panel's Height slider. That
    /// is what makes them precise rather than merely directional: set the slider
    /// to 1.0 and Trim returns grown ground to exactly neutral, which "ramp
    /// toward the hard floor" could only approximate by eye.
    ///
    /// Which of the two can act is still decided by direction — Grow only
    /// raises, Trim only lowers — so a stroke can never undo the one before it
    /// where the two overlap.
    pub fn target(self, brush_height: f32) -> f32 {
        match self {
            Self::Paint => 1.0,
            Self::Erase => 0.0,
            Self::Grow | Self::Trim => brush_height.clamp(MIN_HEIGHT_SCALE, MAX_HEIGHT_SCALE),
        }
    }
}

/// Brush settings for foliage painting.
#[derive(Resource, Clone)]
pub struct FoliagePaintSettings {
    pub active_type: usize,
    pub brush_type: FoliageBrushType,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub brush_falloff: f32,
    /// Blade-height multiplier the Grow and Trim brushes aim for. Ignored by
    /// Paint and Erase, which work the density channel.
    pub brush_height: f32,
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
            // Above neutral, not at it. At 1.0 both height brushes would be
            // no-ops on unpainted ground, so the first click of Grow on a fresh
            // scene would do nothing and read as a broken tool.
            brush_height: 2.0,
            brush_shape: crate::data::BrushShape::Circle,
            falloff_type: crate::data::BrushFalloffType::Smooth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The height channel is lazily allocated, so the overwhelmingly common case
    /// is an empty vector — every chunk nobody has run a height brush over, and
    /// every scene saved before the channel existed. Reading one has to be
    /// neutral rather than zero: zero would collapse every blade on the map.
    #[test]
    fn an_unpainted_height_channel_reads_as_neutral() {
        let map = FoliageDensityMap::new(64);
        assert!(map.height_scale.is_empty(), "should not allocate up front");
        for (u, v) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.13, 0.87)] {
            assert_eq!(map.height_at(u, v), NEUTRAL_HEIGHT_SCALE);
        }
    }

    /// A scene written before `height_scale` existed has no such field. Serde
    /// has to fill it with the empty vector that means "neutral", not fail the
    /// load — this is the whole reason the neutral value is encoded as absence.
    #[test]
    fn a_map_without_the_height_field_still_deserializes() {
        let json = r#"{"resolution":4,"density_weights":[]}"#;
        let map: FoliageDensityMap = serde_json::from_str(json).expect("should load");
        assert!(map.height_scale.is_empty());
        assert_eq!(map.height_at(0.5, 0.5), NEUTRAL_HEIGHT_SCALE);
    }

    /// The serde path is only half of it. Scenes reconstruct components through
    /// `FromReflect`, which fails outright on a field that isn't in the data
    /// unless that field is marked `#[reflect(default)]` — so a new field
    /// carrying only `#[serde(default)]` kills every scene saved before it
    /// existed, at load, with "couldn't create an instance of
    /// `FoliageDensityMap`". Which is exactly what shipping this without the
    /// reflect attribute did.
    #[test]
    fn a_scene_without_the_height_field_reconstructs_by_reflection() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;

        // What an old scene deserializes to: the fields the component had when
        // it was saved, and nothing at all for the one added since.
        let mut old = DynamicStruct::default();
        old.insert("resolution", 4u32);
        old.insert("density_weights", vec![[0.0f32; MAX_FOLIAGE_TYPES]; 16]);

        let map = FoliageDensityMap::from_reflect(&old)
            .expect("a missing height channel must default, not fail the load");
        assert_eq!(map.resolution, 4);
        assert!(map.height_scale.is_empty());
        assert_eq!(map.height_at(0.5, 0.5), NEUTRAL_HEIGHT_SCALE);
    }

    #[test]
    fn ensuring_the_height_channel_sizes_it_at_neutral() {
        let mut map = FoliageDensityMap::new(8);
        map.ensure_height_scale();
        assert_eq!(map.height_scale.len(), 64);
        assert!(map.height_scale.iter().all(|h| *h == NEUTRAL_HEIGHT_SCALE));
        // Idempotent — a stroke calls this every frame.
        map.height_scale[0] = 2.0;
        map.ensure_height_scale();
        assert_eq!(map.height_scale[0], 2.0);
    }

    /// Height is sampled bilinearly for the same reason density is: a nearest
    /// tap makes every texel a hard square of its own world size, and a step in
    /// blade height reads as a tile edge cut through the field.
    #[test]
    fn height_is_bilinear_between_texels() {
        let mut map = FoliageDensityMap::new(2);
        map.ensure_height_scale();
        // Texels are at UV 0 and 1 on both axes with resolution 2.
        map.height_scale = vec![1.0, 3.0, 1.0, 3.0];
        assert!((map.height_at(0.0, 0.0) - 1.0).abs() < 1e-5);
        assert!((map.height_at(1.0, 0.0) - 3.0).abs() < 1e-5);
        let mid = map.height_at(0.5, 0.0);
        assert!(
            (mid - 2.0).abs() < 1e-5,
            "midpoint sampled {mid}, expected 2"
        );
    }

    /// Both channels go through one bilinear tap, so a UV can never land on
    /// different texels depending on which one is being read.
    #[test]
    fn density_and_height_agree_on_where_a_uv_lands() {
        let mut map = FoliageDensityMap::new(4);
        map.ensure_height_scale();
        for i in 0..16 {
            // Same pattern in both channels, offset so the two are not trivially
            // equal by construction.
            map.density_weights[i][0] = i as f32 / 15.0;
            map.height_scale[i] = i as f32 / 15.0;
        }
        for (u, v) in [(0.17, 0.83), (0.5, 0.5), (0.99, 0.01)] {
            let d = map.sample(u, v, 0);
            let h = map.height_at(u, v);
            assert!(
                (d - h).abs() < 1e-6,
                "({u}, {v}): density {d} vs height {h}"
            );
        }
    }

    /// The brushes are all one stroke shape differing only in target, so a mode
    /// that reports the wrong one paints the wrong channel to the wrong value.
    #[test]
    fn brush_targets_and_channels_line_up() {
        // The density modes ignore the slider entirely.
        assert_eq!(FoliageBrushType::Paint.target(2.5), 1.0);
        assert_eq!(FoliageBrushType::Erase.target(2.5), 0.0);
        // The height modes are the slider.
        assert_eq!(FoliageBrushType::Grow.target(2.5), 2.5);
        assert_eq!(FoliageBrushType::Trim.target(0.5), 0.5);
        assert!(!FoliageBrushType::Paint.paints_height());
        assert!(!FoliageBrushType::Erase.paints_height());
        assert!(FoliageBrushType::Grow.paints_height());
        assert!(FoliageBrushType::Trim.paints_height());
    }

    /// Trim's floor is a mown lawn, not bare earth — a zero would make the
    /// height brush a second, worse eraser and leave no way back.
    #[test]
    fn the_height_bounds_straddle_neutral() {
        assert!(MIN_HEIGHT_SCALE > 0.0);
        assert!(MIN_HEIGHT_SCALE < NEUTRAL_HEIGHT_SCALE);
        assert!(MAX_HEIGHT_SCALE > NEUTRAL_HEIGHT_SCALE);
    }

    /// A slider dragged past either end must not be able to push a texel outside
    /// the range the scatter is tuned for.
    #[test]
    fn the_height_target_is_clamped_to_the_bounds() {
        assert_eq!(FoliageBrushType::Grow.target(99.0), MAX_HEIGHT_SCALE);
        assert_eq!(FoliageBrushType::Trim.target(-5.0), MIN_HEIGHT_SCALE);
    }

    /// At exactly neutral both height brushes are no-ops on unpainted ground,
    /// so the first click of Grow on a fresh scene would do nothing at all and
    /// read as a broken tool. The default has to sit off neutral.
    #[test]
    fn the_default_height_target_does_something_on_fresh_ground() {
        let default = FoliagePaintSettings::default().brush_height;
        assert!(default > NEUTRAL_HEIGHT_SCALE);
        assert!(default <= MAX_HEIGHT_SCALE);
    }
}
