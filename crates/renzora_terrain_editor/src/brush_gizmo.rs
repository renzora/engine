//! The terrain brush cursor — the ring that hugs the sculpted surface.
//!
//! Both terrain brush tools raycast the chunk meshes to find where the cursor
//! is, so both know the point they are acting on to the millimetre. What used to
//! differ was what they *drew* there. Sculpt sampled the heightmap all the way
//! round its ring, so the cursor lay on the ground; paint drew a flat circle at
//! the hit point's own Y, which is only correct on level ground and floats free
//! of the terrain — or sinks into it — the moment you paint a slope. It also
//! drew a circle regardless of the brush shape, and nothing at all for the
//! falloff, even though the toolbar offers both for paint exactly as it does for
//! sculpt.
//!
//! So the cursor lives here, once, and both tools call it. A brush setting that
//! the toolbar exposes but the cursor ignores is a setting you have to discover
//! by painting and undoing.
//!
//! The heights come from the chunk heightmaps rather than from more raycasts:
//! the ring is 48 points and the falloff ring another 48, and the mesh is
//! already in memory as the array these read.

use bevy::prelude::*;

use renzora_terrain::data::{BrushShape, TerrainChunkData, TerrainData};

/// Points around each ring. Enough that a 200 m brush doesn't read as a
/// polygon, cheap enough to sample the heightmap at every one of them twice a
/// frame.
pub const RING_SEGMENTS: usize = 48;

/// How far above the surface the ring is drawn, in world units. The terrain is
/// an opaque mesh, so a ring exactly on it z-fights; this is the smallest lift
/// that reads as "on the ground" and still clears it.
const RING_LIFT: f32 = 0.15;

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

/// The full brush cursor: the outer ring at `radius`, plus the inner ring at the
/// edge of the brush's full-strength core.
///
/// The inner ring is where the falloff starts, so the two together say how much
/// of the brush is soft. At `falloff >= 0.99` the core has shrunk to nothing and
/// the second ring would sit on the centre point, so it is dropped.
#[allow(clippy::too_many_arguments)]
pub fn draw_brush_cursor(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    shape: BrushShape,
    falloff: f32,
    color: Color,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) {
    draw_ring(
        gizmos,
        center,
        radius,
        shape,
        terrain,
        terrain_pos,
        chunks,
        color,
    );
    if falloff < 0.99 {
        draw_ring(
            gizmos,
            center,
            radius * (1.0 - falloff),
            shape,
            terrain,
            terrain_pos,
            chunks,
            color.with_alpha(0.4),
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
