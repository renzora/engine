//! The brush cursor: the filled patch that hugs the sculpted surface.
//!
//! All three brush tools (sculpt, surface paint, foliage paint) raycast the
//! chunk meshes to find where the cursor is, so all three know the point they
//! are acting on to the millimetre. What used to differ was what they *drew*
//! there. Sculpt sampled the heightmap all the way round its ring, so the
//! cursor lay on the ground; paint and foliage drew a flat circle at the hit
//! point's own Y, which is only correct on level ground and floats free of the
//! terrain (or sinks into it) the moment you paint a slope. They also drew a
//! circle regardless of the brush shape, and nothing at all for the falloff,
//! even though the toolbar offers both for all three tools.
//!
//! So the cursor lives here, once, in the crate all three tools already depend
//! on, and they all call it. A brush setting that the toolbar exposes but the
//! cursor ignores is a setting you have to discover by painting and undoing.
//!
//! **The interior is filled, not just outlined.** An outline says where the
//! brush ends; it says nothing about how hard it bites in the middle, which is
//! the part you are actually aiming. So the core is drawn solid and the fill
//! fades out along the tool's own falloff curve, which makes the cursor a
//! picture of the weight the stroke will apply rather than a picture of its
//! radius. Bevy gizmos cannot fill a polygon, so the patch is concentric rings
//! packed tightly enough to read as one surface (see [`FILL_SPACING_PX`]).
//!
//! The heights come from the chunk heightmaps rather than from more raycasts:
//! the rings are hundreds of points, and the mesh is already in memory as the
//! array these read.

use bevy::prelude::*;

use crate::data::{
    compute_brush_falloff, BrushFalloffType, BrushShape, TerrainChunkData, TerrainData,
};

/// Points around each ring. Enough that a 200 m brush doesn't read as a
/// polygon, cheap enough to sample the heightmap at every one of them twice a
/// frame.
pub const RING_SEGMENTS: usize = 48;

/// How far above the surface the ring is drawn, in world units. The terrain is
/// an opaque mesh, so a ring exactly on it z-fights; this is the smallest lift
/// that reads as "on the ground" and still clears it.
const RING_LIFT: f32 = 0.15;

/// The fill sits just under the outline's lift, so the two never fight where a
/// fill ring lands on the outline's own radius.
const FILL_LIFT: f32 = RING_LIFT * 0.6;

/// Target gap between two fill rings, in render-target pixels. Gizmo lines are
/// two pixels wide, so rings this close overlap and the patch reads as one
/// surface instead of as a dartboard. It is a *pixel* spacing on purpose: the
/// brush is a fixed size in metres and an arbitrary one on screen, so a ring
/// count fixed in world units is either a wasteful thousand rings up close or a
/// visible set of stripes from far away.
const FILL_SPACING_PX: f32 = 2.0;

/// Ring counts the pixel spacing is clamped between. The floor keeps a brush
/// that is a few pixels across from collapsing to nothing; the ceiling caps the
/// per-frame heightmap sampling when you fill the screen with one brush.
const MIN_FILL_RINGS: usize = 4;
const MAX_FILL_RINGS: usize = 96;

/// Ring count used when the camera can't be asked (the cursor projected behind
/// the camera, or the caller has no camera to hand). Dense enough to look like
/// a fill at an ordinary working distance.
const FALLBACK_FILL_RINGS: usize = 32;

/// Alpha of the fill where the brush is at full strength. Short of 1.0 because
/// the point of painting is watching the ground you are painting: the cursor
/// has to say "all of this, at full weight" without hiding the texture you are
/// deciding to cover.
const FILL_ALPHA: f32 = 0.5;

/// Everything the cursor draws from. Grouped into a struct because the tools
/// pass seven brush settings plus a colour, and a free function taking ten
/// positional arguments is one transposed pair away from a brush that silently
/// draws someone else's falloff.
#[derive(Clone, Copy, Debug)]
pub struct BrushCursor {
    /// World-space point under the mouse, on the terrain surface.
    pub center: Vec3,
    /// Brush radius in world units. Paint and foliage store theirs as a
    /// fraction of a chunk, so they scale it before building this.
    pub radius: f32,
    pub shape: BrushShape,
    pub falloff: f32,
    pub falloff_type: BrushFalloffType,
    /// Outline colour. The fill is this colour at [`FILL_ALPHA`] scaled by the
    /// brush weight, so a tool only picks one colour.
    pub color: Color,
    /// Render-target pixels one world unit spans at the cursor, from
    /// [`pixels_per_unit`]. `None` falls back to [`FALLBACK_FILL_RINGS`].
    pub pixels_per_unit: Option<f32>,
}

/// How many render-target pixels one world unit spans at `at`.
///
/// Measured by projecting two points a metre apart across the camera's own
/// right axis rather than derived from the projection, so it is correct for a
/// perspective and an orthographic camera without either being a special case.
pub fn pixels_per_unit(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    at: Vec3,
) -> Option<f32> {
    let a = camera.world_to_viewport(camera_transform, at).ok()?;
    let b = camera
        .world_to_viewport(camera_transform, at + camera_transform.right() * 1.0)
        .ok()?;
    let px = (b - a).length();
    (px.is_finite() && px > 0.0).then_some(px)
}

/// Rings the fill needs to look continuous at the size it is on screen.
pub fn fill_ring_count(radius: f32, pixels_per_unit: Option<f32>) -> usize {
    let Some(ppu) = pixels_per_unit else {
        return FALLBACK_FILL_RINGS;
    };
    let radius_px = radius * ppu;
    ((radius_px / FILL_SPACING_PX).ceil() as usize).clamp(MIN_FILL_RINGS, MAX_FILL_RINGS)
}

/// Points to walk a fill ring at `ring` of `count` with.
///
/// Proportional to the ring's own radius: the innermost rings are a few pixels
/// across and spending 48 heightmap samples on them buys nothing, while the
/// outermost needs the full count or the fill's edge goes polygonal inside a
/// round outline. Rounded to a multiple of four so a square or diamond still
/// lands on its corners (see [`ring_offset`]).
fn fill_ring_segments(ring: usize, count: usize) -> usize {
    let scaled = RING_SEGMENTS * ring / count.max(1);
    (scaled / 4).clamp(2, RING_SEGMENTS / 4) * 4
}

/// The terrain's composed surface height at a world XZ, or `None` when the
/// point is off the chunk grid.
///
/// Bilinear between the four surrounding vertices — the ring is a smooth curve
/// and stepping it to the nearest vertex makes the cursor visibly stair-step as
/// you drag across a slope. Reads `heights` (the composed buffer the mesh is
/// built from), not `base_heights`, so the cursor follows what you can see
/// including any carve layers.
pub fn surface_height(
    world_x: f32,
    world_z: f32,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) -> Option<f32> {
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;
    let local_x = world_x - terrain_pos.x + half_w;
    let local_z = world_z - terrain_pos.z + half_d;

    let cx = (local_x / terrain.chunk_size).floor() as i32;
    let cz = (local_z / terrain.chunk_size).floor() as i32;
    if cx < 0 || cz < 0 || cx >= terrain.chunks_x as i32 || cz >= terrain.chunks_z as i32 {
        return None;
    }
    let (cx, cz) = (cx as u32, cz as u32);

    let chunk = chunks
        .iter()
        .find(|c| c.chunk_x == cx && c.chunk_z == cz)?;

    let res = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let fx = (local_x - cx as f32 * terrain.chunk_size) / spacing;
    let fz = (local_z - cz as f32 * terrain.chunk_size) / spacing;

    let vx0 = (fx.floor().max(0.0) as u32).min(res - 1);
    let vz0 = (fz.floor().max(0.0) as u32).min(res - 1);
    let vx1 = (vx0 + 1).min(res - 1);
    let vz1 = (vz0 + 1).min(res - 1);
    let tx = fx - fx.floor();
    let tz = fz - fz.floor();

    let h00 = chunk.get_height(vx0, vz0, res);
    let h10 = chunk.get_height(vx1, vz0, res);
    let h01 = chunk.get_height(vx0, vz1, res);
    let h11 = chunk.get_height(vx1, vz1, res);

    let h0 = h00 * (1.0 - tx) + h10 * tx;
    let h1 = h01 * (1.0 - tx) + h11 * tx;
    let normalized = h0 * (1.0 - tz) + h1 * tz;

    Some(terrain.min_height + normalized * terrain.height_range() + terrain_pos.y)
}

/// Where a ring point sits relative to the brush centre, on the XZ plane.
///
/// `t` runs 0..1 once around. The three shapes are parameterised so that a
/// point at the same `t` is at the same *bearing* for all of them, which is what
/// makes switching shape mid-hover look like the ring morphing rather than
/// jumping.
pub fn ring_offset(shape: BrushShape, t: f32, radius: f32) -> Vec2 {
    let angle = t * std::f32::consts::TAU;
    match shape {
        BrushShape::Circle => {
            let (sin_a, cos_a) = angle.sin_cos();
            Vec2::new(cos_a * radius, sin_a * radius)
        }
        // Walk the perimeter side by side rather than solving for the
        // intersection of a ray with the box: it distributes points evenly along
        // the edges instead of bunching them at the corners.
        BrushShape::Square => {
            let s = t * 4.0;
            let frac = s.fract();
            match s.floor() as i32 % 4 {
                0 => Vec2::new(radius, (frac * 2.0 - 1.0) * radius),
                1 => Vec2::new((1.0 - frac * 2.0) * radius, radius),
                2 => Vec2::new(-radius, (1.0 - frac * 2.0) * radius),
                _ => Vec2::new((frac * 2.0 - 1.0) * radius, -radius),
            }
        }
        BrushShape::Diamond => {
            let s = t * 4.0;
            let frac = s.fract();
            match s.floor() as i32 % 4 {
                0 => Vec2::new((1.0 - frac) * radius, frac * radius),
                1 => Vec2::new(-frac * radius, (1.0 - frac) * radius),
                2 => Vec2::new(-(1.0 - frac) * radius, -frac * radius),
                _ => Vec2::new(frac * radius, -(1.0 - frac) * radius),
            }
        }
    }
}

/// Draw one ring at `radius`, riding the terrain surface, and return its points.
///
/// A point that falls off the grid keeps the centre's height rather than being
/// dropped: a ring with a gap in it at the terrain's edge reads as a bug.
#[allow(clippy::too_many_arguments)]
pub fn draw_ring(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    shape: BrushShape,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
    color: Color,
) -> Vec<Vec3> {
    let points: Vec<Vec3> = (0..RING_SEGMENTS)
        .map(|i| {
            let o = ring_offset(shape, i as f32 / RING_SEGMENTS as f32, radius);
            let (wx, wz) = (center.x + o.x, center.z + o.y);
            let y = surface_height(wx, wz, terrain, terrain_pos, chunks).unwrap_or(center.y);
            Vec3::new(wx, y + RING_LIFT, wz)
        })
        .collect();

    for i in 0..RING_SEGMENTS {
        gizmos.line(points[i], points[(i + 1) % RING_SEGMENTS], color);
    }
    points
}

/// Fill the brush's interior, shaded by the weight the stroke will apply there.
///
/// Concentric rings from the centre out, each drawn at the falloff weight of its
/// own radius, so the full-strength core comes out solid and the soft edge fades
/// into the outline. The outermost ring is weight zero by definition and is not
/// drawn: the outline is already there and a second line on top of it only
/// z-fights.
///
/// Points that fall off the chunk grid keep the centre's height, the same
/// fallback [`draw_ring`] makes, so the fill has no holes at the terrain's edge.
fn draw_brush_fill(
    gizmos: &mut Gizmos,
    cursor: &BrushCursor,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) {
    if cursor.radius <= 0.0 {
        return;
    }
    let base_alpha = cursor.color.alpha();
    let rings = fill_ring_count(cursor.radius, cursor.pixels_per_unit);

    for ring in 1..=rings {
        let t = ring as f32 / rings as f32;
        let weight = compute_brush_falloff(t, cursor.falloff, cursor.falloff_type);
        if weight <= 0.001 {
            continue;
        }
        let color = cursor.color.with_alpha(base_alpha * FILL_ALPHA * weight);
        let radius = cursor.radius * t;
        let segments = fill_ring_segments(ring, rings);

        let point = |i: usize| {
            let o = ring_offset(cursor.shape, i as f32 / segments as f32, radius);
            let (wx, wz) = (cursor.center.x + o.x, cursor.center.z + o.y);
            let y = surface_height(wx, wz, terrain, terrain_pos, chunks).unwrap_or(cursor.center.y);
            Vec3::new(wx, y + FILL_LIFT, wz)
        };

        let first = point(0);
        let mut prev = first;
        for i in 1..segments {
            let next = point(i);
            gizmos.line(prev, next, color);
            prev = next;
        }
        gizmos.line(prev, first, color);
    }
}

/// The full brush cursor: the shaded interior, the outer ring at `radius`, and
/// the inner ring at the edge of the brush's full-strength core.
///
/// The inner ring is where the falloff starts, so the two together say how much
/// of the brush is soft. At `falloff >= 0.99` the core has shrunk to nothing and
/// the second ring would sit on the centre point, so it is dropped.
pub fn draw_brush_cursor(
    gizmos: &mut Gizmos,
    cursor: &BrushCursor,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) {
    draw_brush_fill(gizmos, cursor, terrain, terrain_pos, chunks);
    draw_ring(
        gizmos,
        cursor.center,
        cursor.radius,
        cursor.shape,
        terrain,
        terrain_pos,
        chunks,
        cursor.color,
    );
    if cursor.falloff < 0.99 {
        draw_ring(
            gizmos,
            cursor.center,
            cursor.radius * (1.0 - cursor.falloff),
            cursor.shape,
            terrain,
            terrain_pos,
            chunks,
            cursor.color.with_alpha(0.4),
        );
    }
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

    /// Every shape has to close: the last point must lead back to the first, or
    /// the ring shows a notch at t = 0.
    #[test]
    fn every_ring_closes() {
        for shape in [BrushShape::Circle, BrushShape::Square, BrushShape::Diamond] {
            let first = ring_offset(shape, 0.0, 10.0);
            let last = ring_offset(shape, 1.0 - 1.0 / RING_SEGMENTS as f32, 10.0);
            let step = (first - last).length();
            assert!(step < 4.0, "{shape:?} leaves a {step} gap at the seam");
        }
    }

    #[test]
    fn a_circle_stays_at_its_radius() {
        for i in 0..RING_SEGMENTS {
            let o = ring_offset(BrushShape::Circle, i as f32 / RING_SEGMENTS as f32, 7.5);
            assert!((o.length() - 7.5).abs() < 1e-4);
        }
    }

    /// A square's points sit on the box and reach its corners; a shape that
    /// merely bulges toward them is the bug this catches.
    #[test]
    fn a_square_traces_its_box() {
        let r = 5.0;
        let mut max = Vec2::ZERO;
        for i in 0..RING_SEGMENTS {
            let o = ring_offset(BrushShape::Square, i as f32 / RING_SEGMENTS as f32, r);
            assert!(o.abs().max_element() <= r + 1e-4, "{o:?} outside the box");
            assert!(
                (o.x.abs() - r).abs() < 1e-4 || (o.y.abs() - r).abs() < 1e-4,
                "{o:?} is not on an edge"
            );
            max = max.max(o.abs());
        }
        assert!((max - Vec2::splat(r)).abs().max_element() < 1e-4);
    }

    /// A diamond is the L1 ball: every point is `radius` in Manhattan distance.
    #[test]
    fn a_diamond_traces_its_rhombus() {
        let r = 6.0;
        for i in 0..RING_SEGMENTS {
            let o = ring_offset(BrushShape::Diamond, i as f32 / RING_SEGMENTS as f32, r);
            assert!((o.x.abs() + o.y.abs() - r).abs() < 1e-4, "{o:?}");
        }
    }

    /// Zero radius must not blow up — the brush size slider bottoms out and the
    /// inner falloff ring reaches zero on its own at falloff 1.
    #[test]
    fn a_zero_radius_ring_collapses_to_the_centre() {
        for shape in [BrushShape::Circle, BrushShape::Square, BrushShape::Diamond] {
            for i in 0..RING_SEGMENTS {
                let o = ring_offset(shape, i as f32 / RING_SEGMENTS as f32, 0.0);
                assert_eq!(o, Vec2::ZERO, "{shape:?}");
            }
        }
    }

    /// The fill's ring count tracks the brush's size *on screen*, so the same
    /// brush is neither striped when it is far away nor a thousand rings when
    /// you lean into it.
    #[test]
    fn the_fill_packs_rings_by_screen_size() {
        // 10 m at 10 px/m is 100 px of radius, one ring every 2 px.
        assert_eq!(fill_ring_count(10.0, Some(10.0)), 50);
        // The same brush seen from far enough away that it is 8 px across.
        assert_eq!(fill_ring_count(10.0, Some(0.4)), 4);
        // Clamped at both ends, and a camera that couldn't be asked still fills.
        assert_eq!(fill_ring_count(500.0, Some(50.0)), MAX_FILL_RINGS);
        assert_eq!(fill_ring_count(0.01, Some(1.0)), MIN_FILL_RINGS);
        assert_eq!(fill_ring_count(10.0, None), FALLBACK_FILL_RINGS);
    }

    /// Inner rings are cheaper than outer ones, but never so cheap that a square
    /// loses its corners: the count stays a multiple of four.
    #[test]
    fn fill_rings_scale_their_segments_and_keep_corners() {
        let rings = 32;
        let mut prev = 0;
        for ring in 1..=rings {
            let segs = fill_ring_segments(ring, rings);
            assert_eq!(segs % 4, 0, "ring {ring} has {segs} segments");
            assert!((8..=RING_SEGMENTS).contains(&segs), "ring {ring}: {segs}");
            assert!(segs >= prev, "segment count went backwards at ring {ring}");
            prev = segs;
        }
        assert_eq!(fill_ring_segments(rings, rings), RING_SEGMENTS);
    }

    /// The whole point of the fill: full weight in the middle, nothing at the
    /// rim, so the patch reads as the brush's strength and not just its extent.
    #[test]
    fn the_fill_is_solid_in_the_core_and_gone_at_the_rim() {
        let rings = 20;
        let weight = |ring: usize| {
            compute_brush_falloff(
                ring as f32 / rings as f32,
                0.5,
                BrushFalloffType::Smooth,
            )
        };
        assert_eq!(weight(1), 1.0);
        assert_eq!(weight(rings / 2), 1.0);
        assert!(weight(rings * 3 / 4) < 1.0);
        assert_eq!(weight(rings), 0.0);
    }

    #[test]
    fn height_is_none_off_the_grid() {
        let t = terrain();
        let chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        let chunks = [&chunk];
        assert!(surface_height(-500.0, 0.0, &t, Vec3::ZERO, &chunks).is_none());
        assert!(surface_height(0.0, 500.0, &t, Vec3::ZERO, &chunks).is_none());
    }

    /// A missing chunk is `None`, not a silent zero — the ring falls back to the
    /// centre's height rather than diving to the terrain floor.
    #[test]
    fn height_is_none_when_the_chunk_is_absent() {
        let t = terrain();
        let chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        // (1, 1) exists in the grid but isn't in the slice.
        assert!(surface_height(60.0, 60.0, &t, Vec3::ZERO, &[&chunk]).is_none());
    }

    /// A flat chunk reads back its own level, and the terrain's own Y offset is
    /// carried through — the cursor sits on a terrain you have moved, not on the
    /// height it would have had at the origin.
    #[test]
    fn a_flat_chunk_reads_its_level_and_follows_the_terrain() {
        let t = terrain();
        let chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        let chunks = [&chunk];
        let expect = t.min_height + 0.5 * t.height_range();
        // Terrain-local (0,0) is the grid's minimum corner, which in world space
        // is half the extent away from the entity's own position.
        let (wx, wz) = (-t.total_width() / 2.0 + 1.0, -t.total_depth() / 2.0 + 1.0);
        let flat = surface_height(wx, wz, &t, Vec3::ZERO, &chunks).unwrap();
        assert!((flat - expect).abs() < 1e-3, "{flat} != {expect}");

        let lifted = surface_height(wx, wz, &t, Vec3::new(0.0, 12.0, 0.0), &chunks).unwrap();
        assert!((lifted - (expect + 12.0)).abs() < 1e-3);
    }

    /// Bilinear, not nearest: a point between two vertices of different heights
    /// must land strictly between them, or the ring stair-steps down a slope.
    #[test]
    fn heights_interpolate_between_vertices() {
        let t = terrain();
        let res = t.chunk_resolution;
        let mut chunk = TerrainChunkData::new(0, 0, res, 0.0);
        // A ramp along X, so a half-way sample has an unambiguous answer.
        for vz in 0..res {
            for vx in 0..res {
                chunk.heights[(vz * res + vx) as usize] = vx as f32 / (res - 1) as f32;
            }
        }
        let chunks = [&chunk];
        let spacing = t.vertex_spacing();
        let base_x = -t.total_width() / 2.0;
        let z = -t.total_depth() / 2.0 + 1.0;
        let at = |x: f32| surface_height(x, z, &t, Vec3::ZERO, &chunks).unwrap();
        let (a, mid, b) = (
            at(base_x + spacing * 3.0),
            at(base_x + spacing * 3.5),
            at(base_x + spacing * 4.0),
        );
        assert!(a < mid && mid < b, "{a} / {mid} / {b} is not interpolated");
        assert!(((a + b) / 2.0 - mid).abs() < 1e-3);
    }
}
