//! Whole-terrain procedural generation — build a mountain range in one action
//! instead of painting one.
//!
//! The sculpt brushes in [`crate::sculpt`] are the right tool for shaping a
//! hillside you can see. They are the wrong tool for the first five minutes of a
//! level, where what you want is *a landscape* — and dragging the Noise brush
//! back and forth over a 256 m terrain to get one is neither fast nor
//! repeatable: the result depends on where the strokes happened to overlap, and
//! there is no way to say "same range, but bigger peaks" without starting over.
//!
//! This module is the other half: one parameter set, evaluated over a rectangle,
//! applied in a single pass. Change a number and re-apply and you get the same
//! terrain with that one thing different, because nothing about the result
//! depends on mouse history.
//!
//! # Coordinate space
//!
//! Everything here is in **terrain-local metres with the origin at the grid's
//! minimum corner** — the same space [`crate::sculpt::apply_brush`] takes its
//! `local_x` / `local_z` in, and the space `chunk_x * chunk_size` addresses.
//! It is deliberately *not* world space: sampling in world space would mean
//! sliding the terrain entity re-rolled the whole landscape, which is a nasty
//! surprise for something you positioned by hand.
//!
//! # The one non-obvious parameter
//!
//! [`TerrainGenSettings::feather`] exists because a region that isn't the whole
//! terrain has to *meet* the terrain around it. Without it, Replace leaves a
//! vertical wall at the region border — the generated heights simply stop. The
//! feather band is a weight ramp, not a height ramp, so it blends toward
//! whatever is already there rather than toward a fixed level, and a region
//! generated over an existing hillside still lands on that hillside.

use bevy::prelude::*;

use crate::data::{NoiseMode, StampBlendMode, TerrainChunkData, TerrainData};
use crate::sculpt::eval_noise;

/// Region rectangle in terrain-local metres, origin at the grid's minimum
/// corner. `min` is always componentwise ≤ `max`; [`GenRegion::normalized`]
/// enforces it after a drag that crossed over itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenRegion {
    pub min: Vec2,
    pub max: Vec2,
}

impl GenRegion {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }.normalized()
    }

    /// The whole terrain — the default region, and what Reset returns to.
    pub fn whole(terrain: &TerrainData) -> Self {
        Self {
            min: Vec2::ZERO,
            max: Vec2::new(terrain.total_width(), terrain.total_depth()),
        }
    }

    /// Swap any inverted axis. Dragging a corner past its opposite is a normal
    /// thing to do and must produce a rectangle, not an empty one.
    pub fn normalized(self) -> Self {
        Self {
            min: self.min.min(self.max),
            max: self.min.max(self.max),
        }
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Clamp to the terrain's extent. Generating outside the grid writes
    /// nothing, so a region that has wandered off it is only ever confusing.
    pub fn clamped_to(self, terrain: &TerrainData) -> Self {
        let hi = Vec2::new(terrain.total_width(), terrain.total_depth());
        Self {
            min: self.min.clamp(Vec2::ZERO, hi),
            max: self.max.clamp(Vec2::ZERO, hi),
        }
        .normalized()
    }
}

/// The generator's parameters. A resource rather than a component: like
/// [`crate::data::TerrainSettings`] it is *tool* state, shared by whichever
/// terrain is in hand, and it is not part of the saved scene.
#[derive(Resource, Clone, Debug)]
pub struct TerrainGenSettings {
    /// When set, the region follows the terrain's extent and the stored
    /// rectangle is ignored — so growing the terrain with the Region tool
    /// grows what Generate covers, without a second edit. Dragging a handle
    /// clears it (see [`TerrainGenSettings::set_region`]).
    pub whole_terrain: bool,
    /// The rectangle, in terrain-local metres. Only meaningful while
    /// `whole_terrain` is false; read it through
    /// [`TerrainGenSettings::region`], which handles both cases.
    pub region_min: Vec2,
    pub region_max: Vec2,

    pub mode: NoiseMode,
    /// Metres per unit of noise — the size of the largest features. This is the
    /// dial that decides "alpine range" from "gravel".
    pub scale: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub persistence: f32,
    /// Only read by [`NoiseMode::Warped`].
    pub warp_strength: f32,
    pub seed: u32,
    /// Raises the normalized noise to this power before it becomes a height.
    /// Above 1 it pushes the midrange down, which turns the rolling lumps that
    /// raw FBM produces into peaks separated by valley floors; below 1 it
    /// flattens the tops into plateaus. It is the cheapest single thing that
    /// makes generated terrain read as mountains rather than as noise.
    pub exponent: f32,

    /// Elevation the noise floor sits at, in world metres.
    pub base: f32,
    /// Peak-to-floor amplitude, in world metres. Clamped against the terrain's
    /// own height range at apply time — you cannot generate past `max_height`.
    pub height: f32,

    /// How the result combines with what is already there.
    pub blend: StampBlendMode,
    /// Width of the edge blend, as a fraction of the region's half-extent.
    /// 0 is a hard edge; 1 feathers from the border all the way to the centre.
    pub feather: f32,

    /// Draw the wireframe preview of the result over the scene.
    pub preview: bool,
}

impl Default for TerrainGenSettings {
    fn default() -> Self {
        Self {
            whole_terrain: true,
            region_min: Vec2::ZERO,
            region_max: Vec2::splat(256.0),
            // Hybrid is ridged noise with FBM mixed back in: ridges give the
            // sharp crest lines that read as mountains, the FBM keeps the
            // flanks from looking machined. It is the mode this feature exists
            // for, so it is the one you get without choosing.
            mode: NoiseMode::Hybrid,
            scale: 120.0,
            octaves: 6,
            lacunarity: 2.0,
            persistence: 0.5,
            warp_strength: 0.5,
            seed: 1337,
            exponent: 1.6,
            base: 0.0,
            height: 30.0,
            // Replace, not Add: the common case is "give me a landscape",
            // starting from the flat terrain you just spawned. Add is there for
            // laying a range over ground you have already sculpted.
            blend: StampBlendMode::Replace,
            feather: 0.35,
            preview: true,
        }
    }
}

impl TerrainGenSettings {
    /// The active rectangle — the terrain's extent while `whole_terrain` is on,
    /// otherwise the stored one clamped to the grid.
    pub fn region(&self, terrain: &TerrainData) -> GenRegion {
        if self.whole_terrain {
            GenRegion::whole(terrain)
        } else {
            GenRegion::new(self.region_min, self.region_max).clamped_to(terrain)
        }
    }

    /// Store an explicit rectangle. Clears `whole_terrain`, because a rectangle
    /// the user dragged that silently snapped back to the terrain extent next
    /// frame is worse than no drag at all.
    pub fn set_region(&mut self, region: GenRegion) {
        let r = region.normalized();
        self.region_min = r.min;
        self.region_max = r.max;
        self.whole_terrain = false;
    }

    /// Effective amplitude, in metres — never more than the terrain's own
    /// height range, since heights are stored normalized to it and anything
    /// past the top would just clip flat.
    pub fn effective_height(&self, terrain: &TerrainData) -> f32 {
        self.height.min(terrain.height_range().max(0.0))
    }
}

/// Cheap smoothstep on an already-clamped `t`.
#[inline]
fn smoothstep01(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// How strongly the generator applies at a point: 1 in the region's core, 0
/// outside it, smoothstepped across the feather band in between.
///
/// The two axes are combined with `min` rather than multiplied, so a point that
/// is deep inside on one axis and mid-feather on the other gets the feather
/// weight — multiplying would darken the corners into a visible pinch.
pub fn region_weight(region: &GenRegion, feather: f32, x: f32, z: f32) -> f32 {
    let inset_x = (x - region.min.x).min(region.max.x - x);
    let inset_z = (z - region.min.y).min(region.max.y - z);
    if inset_x < 0.0 || inset_z < 0.0 {
        return 0.0;
    }
    let f = feather.clamp(0.0, 1.0);
    if f <= 0.0 {
        return 1.0;
    }
    let size = region.size();
    let band_x = size.x * 0.5 * f;
    let band_z = size.y * 0.5 * f;
    let wx = if band_x > 0.0 {
        smoothstep01((inset_x / band_x).clamp(0.0, 1.0))
    } else {
        1.0
    };
    let wz = if band_z > 0.0 {
        smoothstep01((inset_z / band_z).clamp(0.0, 1.0))
    } else {
        1.0
    };
    wx.min(wz)
}

/// The generator's raw output at a point, in world metres, ignoring the region
/// and whatever the terrain already looks like.
pub fn sample_height(g: &TerrainGenSettings, terrain: &TerrainData, x: f32, z: f32) -> f32 {
    let scale = g.scale.max(0.1);
    let n = eval_noise(
        x / scale,
        z / scale,
        g.mode,
        g.octaves.clamp(1, 8),
        g.lacunarity,
        g.persistence,
        g.seed,
        g.warp_strength,
    )
    .clamp(0.0, 1.0);
    let shaped = n.powf(g.exponent.max(0.05));
    g.base + shaped * g.effective_height(terrain)
}

/// The height one vertex ends up at, normalized to the terrain's range.
///
/// This is the whole algorithm, and it is a free function taking `current`
/// precisely so the viewport preview and the apply pass can call the same code.
/// When they were two implementations the preview drifted from the result the
/// moment a blend mode was added, and a preview you cannot trust is worse than
/// none.
pub fn blended_height(
    g: &TerrainGenSettings,
    terrain: &TerrainData,
    region: &GenRegion,
    current: f32,
    x: f32,
    z: f32,
) -> f32 {
    let w = region_weight(region, g.feather, x, z);
    if w <= 0.0 {
        return current;
    }
    let range = terrain.height_range();
    if range <= 0.0 {
        return current;
    }
    let elevation = sample_height(g, terrain, x, z);
    // Absolute height, for the modes that place terrain at an elevation…
    let absolute = (elevation - terrain.min_height) / range;
    // …and the part above the floor, for the modes that add to what's there.
    let relative = (elevation - g.base) / range;

    let target = match g.blend {
        StampBlendMode::Replace => absolute,
        StampBlendMode::Add => current + relative,
        StampBlendMode::Subtract => current - relative,
        StampBlendMode::Max => current.max(absolute),
        StampBlendMode::Min => current.min(absolute),
    };
    (current + (target - current) * w).clamp(0.0, 1.0)
}

/// Run the generator over one chunk's base heightmap.
///
/// Writes to `base_heights` — the user's sculpt layer — so a generated
/// landscape is something the brushes then carve into, and the height-layer
/// stack composes on top of it exactly as it does for hand-sculpted ground.
///
/// Returns whether anything actually moved, so the caller can skip flagging
/// chunks the region never touched.
pub fn apply_to_chunk(
    chunk: &mut TerrainChunkData,
    terrain: &TerrainData,
    g: &TerrainGenSettings,
    region: &GenRegion,
) -> bool {
    let resolution = terrain.chunk_resolution;
    if resolution == 0 {
        return false;
    }
    let spacing = terrain.vertex_spacing();
    let origin_x = chunk.chunk_x as f32 * terrain.chunk_size;
    let origin_z = chunk.chunk_z as f32 * terrain.chunk_size;

    // Whole-chunk reject. On a large grid most chunks miss a small region, and
    // this turns the pass from resolution² per chunk into one comparison.
    if origin_x > region.max.x
        || origin_x + terrain.chunk_size < region.min.x
        || origin_z > region.max.y
        || origin_z + terrain.chunk_size < region.min.y
    {
        return false;
    }

    let mut changed = false;
    for vz in 0..resolution {
        for vx in 0..resolution {
            let idx = (vz * resolution + vx) as usize;
            let Some(&current) = chunk.base_heights.get(idx) else {
                continue;
            };
            let x = origin_x + vx as f32 * spacing;
            let z = origin_z + vz as f32 * spacing;
            let next = blended_height(g, terrain, region, current, x, z);
            if (next - current).abs() > 1e-6 {
                chunk.base_heights[idx] = next;
                changed = true;
            }
        }
    }
    if changed {
        chunk.dirty = true;
    }
    changed
}

/// Advance the seed to a fresh landscape. A plain increment rather than a
/// random draw: re-rolling is something you do repeatedly looking for one you
/// like, and a sequence you can walk back to is worth more here than
/// unpredictability.
pub fn next_seed(seed: u32) -> u32 {
    seed.wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData {
            chunks_x: 2,
            chunks_z: 2,
            chunk_size: 64.0,
            chunk_resolution: 33,
            min_height: -10.0,
            max_height: 40.0,
            ..TerrainData::default()
        }
    }

    #[test]
    fn whole_terrain_region_matches_the_grid_extent() {
        let t = terrain();
        let g = TerrainGenSettings::default();
        let r = g.region(&t);
        assert_eq!(r.min, Vec2::ZERO);
        assert_eq!(r.max, Vec2::new(t.total_width(), t.total_depth()));
    }

    /// Dragging a handle must stick. `whole_terrain` re-deriving the rectangle
    /// every frame would silently undo the drag.
    #[test]
    fn setting_a_region_leaves_whole_terrain_mode() {
        let mut g = TerrainGenSettings::default();
        assert!(g.whole_terrain);
        g.set_region(GenRegion::new(Vec2::new(10.0, 10.0), Vec2::new(40.0, 50.0)));
        assert!(!g.whole_terrain);
        assert_eq!(g.region(&terrain()).min, Vec2::new(10.0, 10.0));
    }

    /// A corner dragged past its opposite is a rectangle, not an empty one.
    #[test]
    fn inverted_drags_normalize() {
        let r = GenRegion::new(Vec2::new(60.0, 80.0), Vec2::new(20.0, 10.0));
        assert_eq!(r.min, Vec2::new(20.0, 10.0));
        assert_eq!(r.max, Vec2::new(60.0, 80.0));
        assert!(r.size().x > 0.0 && r.size().y > 0.0);
    }

    #[test]
    fn region_clamps_to_the_terrain() {
        let t = terrain();
        let r = GenRegion::new(Vec2::new(-50.0, -50.0), Vec2::new(9999.0, 9999.0)).clamped_to(&t);
        assert_eq!(r.min, Vec2::ZERO);
        assert_eq!(r.max, Vec2::new(t.total_width(), t.total_depth()));
    }

    #[test]
    fn weight_is_zero_outside_and_one_at_the_centre() {
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(100.0));
        assert_eq!(region_weight(&r, 0.5, -1.0, 50.0), 0.0);
        assert_eq!(region_weight(&r, 0.5, 101.0, 50.0), 0.0);
        assert!((region_weight(&r, 0.5, 50.0, 50.0) - 1.0).abs() < 1e-5);
    }

    /// The feather has to reach 0 *at* the border, or Replace leaves a wall
    /// there — the exact thing the band exists to prevent.
    #[test]
    fn weight_reaches_zero_at_the_border() {
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(100.0));
        assert!(region_weight(&r, 0.5, 0.0, 50.0) < 1e-5);
        assert!(region_weight(&r, 0.5, 100.0, 50.0) < 1e-5);
    }

    /// A zero feather is a hard edge everywhere inside, including on the border
    /// itself — otherwise "no feather" would still round the corners.
    #[test]
    fn zero_feather_is_a_hard_edge() {
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(100.0));
        assert_eq!(region_weight(&r, 0.0, 0.0, 0.0), 1.0);
        assert_eq!(region_weight(&r, 0.0, 50.0, 50.0), 1.0);
        assert_eq!(region_weight(&r, 0.0, 100.1, 50.0), 0.0);
    }

    #[test]
    fn weight_rises_monotonically_across_the_band() {
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(100.0));
        let mut last = -1.0;
        for i in 0..=25 {
            let w = region_weight(&r, 0.5, i as f32, 50.0);
            assert!(w >= last - 1e-6, "weight dipped at x={i}");
            last = w;
        }
    }

    #[test]
    fn generated_heights_stay_inside_the_terrain_range() {
        let t = terrain();
        let g = TerrainGenSettings {
            height: 10_000.0,
            base: -10_000.0,
            ..Default::default()
        };
        let r = g.region(&t);
        for i in 0..64 {
            let p = i as f32 * 2.0;
            let h = blended_height(&g, &t, &r, 0.5, p, p);
            assert!((0.0..=1.0).contains(&h), "height {h} out of range at {p}");
        }
    }

    /// Amplitude is capped by the terrain, not by the field: asking for 500 m of
    /// mountains on a 50 m terrain gives 50, not a plateau of clipped values.
    #[test]
    fn amplitude_is_capped_by_the_terrain_height_range() {
        let t = terrain();
        let g = TerrainGenSettings {
            height: 500.0,
            ..Default::default()
        };
        assert_eq!(g.effective_height(&t), t.height_range());
    }

    #[test]
    fn a_different_seed_is_a_different_landscape() {
        let t = terrain();
        let a = TerrainGenSettings::default();
        let b = TerrainGenSettings {
            seed: next_seed(a.seed),
            ..a.clone()
        };
        let differs = (0..64).any(|i| {
            let p = i as f32 * 3.0;
            (sample_height(&a, &t, p, p) - sample_height(&b, &t, p, p)).abs() > 1e-4
        });
        assert!(differs, "reseeding produced an identical heightfield");
    }

    /// Same parameters, same terrain — twice. The whole point of generating
    /// rather than painting is that the result is a function of the settings.
    #[test]
    fn generation_is_deterministic() {
        let t = terrain();
        let g = TerrainGenSettings::default();
        for i in 0..32 {
            let p = i as f32 * 5.0;
            assert_eq!(sample_height(&g, &t, p, p), sample_height(&g, &t, p, p));
        }
    }

    #[test]
    fn add_and_subtract_are_opposites() {
        let t = terrain();
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(128.0));
        let add = TerrainGenSettings {
            blend: StampBlendMode::Add,
            feather: 0.0,
            // Small amplitude on purpose: the point is to compare the two
            // directions, and a big one clamps at the top before it can.
            height: 5.0,
            ..Default::default()
        };
        let sub = TerrainGenSettings {
            blend: StampBlendMode::Subtract,
            ..add.clone()
        };
        // Mid-range start so neither direction clamps.
        let start = 0.5;
        let up = blended_height(&add, &t, &r, start, 40.0, 40.0) - start;
        let down = start - blended_height(&sub, &t, &r, start, 40.0, 40.0);
        assert!((up - down).abs() < 1e-5);
    }

    #[test]
    fn max_never_lowers_and_min_never_raises() {
        let t = terrain();
        let r = GenRegion::new(Vec2::ZERO, Vec2::splat(128.0));
        let max = TerrainGenSettings {
            blend: StampBlendMode::Max,
            feather: 0.0,
            ..Default::default()
        };
        let min = TerrainGenSettings {
            blend: StampBlendMode::Min,
            ..max.clone()
        };
        for i in 0..40 {
            let p = i as f32 * 3.0;
            assert!(blended_height(&max, &t, &r, 0.4, p, p) >= 0.4 - 1e-6);
            assert!(blended_height(&min, &t, &r, 0.4, p, p) <= 0.4 + 1e-6);
        }
    }

    /// Nothing outside the rectangle may move, whatever the blend mode — a
    /// region generate that quietly edited the rest of the terrain would make
    /// the tool unusable for anything but the first pass.
    #[test]
    fn points_outside_the_region_are_untouched() {
        let t = terrain();
        let mut g = TerrainGenSettings::default();
        g.set_region(GenRegion::new(Vec2::splat(20.0), Vec2::splat(60.0)));
        let r = g.region(&t);
        for blend in StampBlendMode::all() {
            g.blend = *blend;
            assert_eq!(blended_height(&g, &t, &r, 0.42, 10.0, 40.0), 0.42);
            assert_eq!(blended_height(&g, &t, &r, 0.42, 40.0, 90.0), 0.42);
        }
    }

    /// A chunk the region misses entirely must take the early-out: no writes,
    /// no dirty flag, no mesh rebuild for a chunk that did not change.
    #[test]
    fn chunks_outside_the_region_are_skipped() {
        let t = terrain();
        let mut g = TerrainGenSettings::default();
        g.set_region(GenRegion::new(Vec2::ZERO, Vec2::splat(30.0)));
        let r = g.region(&t);
        let mut far = TerrainChunkData::new(1, 1, t.chunk_resolution, 0.2);
        // A fresh chunk is born dirty (it has never been meshed); clear it so
        // the assertion below is about the generator and not about that.
        far.dirty = false;
        assert!(!apply_to_chunk(&mut far, &t, &g, &r));
        assert!(!far.dirty);
        assert!(far.base_heights.iter().all(|h| (*h - 0.2).abs() < 1e-6));
    }

    #[test]
    fn applying_to_a_covered_chunk_writes_and_flags_it() {
        let t = terrain();
        let g = TerrainGenSettings::default();
        let r = g.region(&t);
        let mut chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.2);
        assert!(apply_to_chunk(&mut chunk, &t, &g, &r));
        assert!(chunk.dirty);
        assert!(chunk.base_heights.iter().any(|h| (*h - 0.2).abs() > 1e-4));
        assert!(chunk.base_heights.iter().all(|h| (0.0..=1.0).contains(h)));
    }

    /// Re-applying the same settings is idempotent under Replace, which is what
    /// makes "tweak a slider, generate again" behave the way it looks like it
    /// should instead of compounding. Only in the region's core: the feather
    /// band blends *toward* the target by design, so it converges over repeated
    /// applications rather than landing in one.
    #[test]
    fn replace_is_idempotent_where_the_weight_is_full() {
        let t = terrain();
        let g = TerrainGenSettings {
            feather: 0.0,
            ..Default::default()
        };
        let r = g.region(&t);
        let mut chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.2);
        apply_to_chunk(&mut chunk, &t, &g, &r);
        let once = chunk.base_heights.clone();
        assert!(!apply_to_chunk(&mut chunk, &t, &g, &r));
        assert_eq!(once, chunk.base_heights);
    }

    /// The exponent is the peaks dial: raising it must lower the midrange,
    /// otherwise it is doing nothing worth a slider.
    #[test]
    fn a_higher_exponent_deepens_the_valleys() {
        let t = terrain();
        let flat = TerrainGenSettings {
            exponent: 1.0,
            ..Default::default()
        };
        let peaky = TerrainGenSettings {
            exponent: 3.0,
            ..flat.clone()
        };
        let mean = |g: &TerrainGenSettings| {
            let n = 40;
            (0..n)
                .map(|i| sample_height(g, &t, i as f32 * 7.0, i as f32 * 5.0))
                .sum::<f32>()
                / n as f32
        };
        assert!(mean(&peaky) < mean(&flat));
    }
}
