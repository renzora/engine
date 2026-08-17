//! Growing and shrinking a terrain's chunk grid one row or column at a time.
//!
//! Setting `chunks_x`/`chunks_z` directly is a blunt instrument: it says how big
//! the grid is, but not *which way* it grew, and the grid is centred on its
//! parent — so `chunk_world_origin` re-centres every chunk the moment the count
//! changes. Add a column and the terrain you already sculpted slides half a chunk
//! sideways under your cursor. That is the bug this module exists to prevent.
//!
//! Two things have to happen together for an edge resize to look like the edge
//! moved rather than the world moving:
//!
//! 1. **Re-index.** Growing on the −X side means what used to be column 0 is now
//!    column 1, so every surviving chunk's `chunk_x` shifts. Growing on +X
//!    appends and shifts nothing.
//! 2. **Compensate.** Whichever way it grew, re-centring moves every chunk half a
//!    chunk in local space; the parent transform moves the opposite way by the
//!    same amount, so the existing terrain lands exactly where it was.
//!
//! [`plan_grid_resize`] is the whole calculation, as a pure function over
//! [`TerrainData`] — it is the part worth testing, and it is tested below.
//! [`apply_grid_resize`] is the thin world-mutating wrapper: re-index, resize,
//! compensate, and let `terrain_data_changed_system` rebuild from there. That
//! system already preserves heights across a grid change by keying them on
//! `(chunk_x, chunk_z)`, which is exactly why step 1 must land *before* it runs.

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Upper bound on either axis. Not a technical limit — a guard rail. At the
/// default 129² resolution a 32×32 grid is already ~17 M vertices and 1024
/// trimesh colliders, which is well past the point where a mistake costs you
/// minutes rather than seconds.
pub const MAX_CHUNKS_PER_AXIS: u32 = 32;

/// What a given grid would cost to build, so the editor can say so *before*
/// building it rather than after.
///
/// This is the number the old Chunks X/Z sliders never showed. Dragging one from
/// 1 to 8 rebuilds the whole terrain at every integer on the way, and at 257²
/// resolution the last of those steps is 4.2 million vertices and 64 trimesh
/// colliders — enough to stall the editor long enough to look like a crash. The
/// estimate is deliberately rough: its job is to separate "fine" from "you are
/// about to wait a long time", not to predict allocator behaviour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TerrainCost {
    pub chunks: u64,
    pub vertices: u64,
    pub triangles: u64,
    /// Heightmaps + vertex buffers + index buffers, in bytes.
    pub bytes: u64,
}

/// Vertices past which the editor warns. Chosen from the collider cost, which is
/// what actually hurts: trimesh construction is the slow part of a rebuild, and
/// a few million vertices' worth is where a resize stops being instant.
pub const COST_WARN_VERTICES: u64 = 2_000_000;

pub fn estimate_cost(chunks_x: u32, chunks_z: u32, resolution: u32) -> TerrainCost {
    let chunks = chunks_x as u64 * chunks_z as u64;
    let per_chunk_verts = resolution as u64 * resolution as u64;
    let quads = (resolution.saturating_sub(1)) as u64;
    let per_chunk_tris = quads * quads * 2;

    let vertices = chunks * per_chunk_verts;
    let triangles = chunks * per_chunk_tris;

    // Per vertex: position + normal (12 each) + UV (8) = 32 render bytes, plus
    // the two f32 height buffers the chunk keeps on the CPU (`base_heights` and
    // the composed `heights`) at 4 bytes each.
    const RENDER_BYTES_PER_VERTEX: u64 = 32;
    const HEIGHT_BYTES_PER_VERTEX: u64 = 8;
    // Indices are u32, three to a triangle.
    const BYTES_PER_INDEX: u64 = 4;

    let bytes = vertices * (RENDER_BYTES_PER_VERTEX + HEIGHT_BYTES_PER_VERTEX)
        + triangles * 3 * BYTES_PER_INDEX;

    TerrainCost {
        chunks,
        vertices,
        triangles,
        bytes,
    }
}

/// Which side of the chunk grid a resize acts on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GridEdge {
    MinX,
    MaxX,
    MinZ,
    MaxZ,
}

impl GridEdge {
    pub fn all() -> &'static [GridEdge] {
        &[Self::MinX, Self::MaxX, Self::MinZ, Self::MaxZ]
    }

    /// True for the two edges that renumber the surviving chunks.
    pub fn is_min(&self) -> bool {
        matches!(self, Self::MinX | Self::MinZ)
    }
}

/// The outcome of a resize, before anything is written.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GridResize {
    pub chunks_x: u32,
    pub chunks_z: u32,
    /// Added to every surviving chunk's `chunk_x` (0 or ±1).
    pub shift_x: i32,
    /// Added to every surviving chunk's `chunk_z` (0 or ±1).
    pub shift_z: i32,
    /// Added to the terrain's **local** translation, cancelling out the
    /// re-centring so already-sculpted chunks don't visibly move.
    pub local_offset: Vec3,
    /// The row/column index that disappears, when shrinking. `None` when growing.
    pub dropped: Option<(GridEdge, u32)>,
}

/// Work out what `delta` chunks on `edge` would do. `delta` is `+1` to grow or
/// `-1` to shrink. Returns `None` when the resize is impossible — past
/// [`MAX_CHUNKS_PER_AXIS`], or down to zero chunks on an axis.
pub fn plan_grid_resize(data: &TerrainData, edge: GridEdge, delta: i32) -> Option<GridResize> {
    if delta != 1 && delta != -1 {
        return None;
    }
    let (mut chunks_x, mut chunks_z) = (data.chunks_x, data.chunks_z);
    let horizontal = matches!(edge, GridEdge::MinX | GridEdge::MaxX);

    // Resize the affected axis, refusing to leave the legal range.
    let axis = if horizontal { chunks_x } else { chunks_z };
    let next = axis as i64 + delta as i64;
    if next < 1 || next > MAX_CHUNKS_PER_AXIS as i64 {
        return None;
    }
    if horizontal {
        chunks_x = next as u32;
    } else {
        chunks_z = next as u32;
    }

    // Only the min-side edges renumber: growing there inserts before index 0,
    // shrinking there removes index 0 and slides everything down.
    let shift = if edge.is_min() { delta } else { 0 };

    // Re-centring moves every chunk by half a chunk in local space; which way
    // depends on both the edge and the direction. Growing on a min edge pushes
    // the existing chunks toward +axis, so the parent goes −axis to cancel it;
    // every other combination flips one of those signs.
    //
    //   grow  min → local +S/2 → compensate −S/2
    //   grow  max → local −S/2 → compensate +S/2
    //   shrink min → local −S/2 → compensate +S/2
    //   shrink max → local +S/2 → compensate −S/2
    let half = data.chunk_size * 0.5;
    let sign = if edge.is_min() { -1.0 } else { 1.0 } * delta as f32;
    let local_offset = if horizontal {
        Vec3::new(sign * half, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, sign * half)
    };

    // Shrinking drops the row/column at the edge. Index it against the *old*
    // dimensions — that's what the live chunk entities are still numbered by.
    let dropped = (delta == -1).then(|| {
        let idx = match edge {
            GridEdge::MinX | GridEdge::MinZ => 0,
            GridEdge::MaxX => data.chunks_x - 1,
            GridEdge::MaxZ => data.chunks_z - 1,
        };
        (edge, idx)
    });

    Some(GridResize {
        chunks_x,
        chunks_z,
        shift_x: if horizontal { shift } else { 0 },
        shift_z: if horizontal { 0 } else { shift },
        local_offset,
        dropped,
    })
}

/// Apply a planned resize to the world: despawn the dropped row (if shrinking),
/// re-index the survivors, compensate the parent transform, then write the new
/// dimensions. Writing `TerrainData` last is deliberate — `terrain_data_changed_system`
/// reads the chunks' indices when it rebuilds, so they must already be correct.
///
/// Returns `false` if the terrain is gone or the resize isn't legal.
pub fn apply_grid_resize(
    world: &mut World,
    terrain: Entity,
    edge: GridEdge,
    delta: i32,
) -> bool {
    let Some(data) = world.get::<TerrainData>(terrain).cloned() else {
        return false;
    };
    let Some(plan) = plan_grid_resize(&data, edge, delta) else {
        return false;
    };

    // Collect this terrain's chunks up front: the loop below mutates them, and
    // the query borrow can't be held across that.
    let chunks: Vec<(Entity, u32, u32)> = world
        .query::<(Entity, &TerrainChunkData, &TerrainChunkOf)>()
        .iter(world)
        .filter(|(_, _, of)| of.0 == terrain)
        .map(|(e, c, _)| (e, c.chunk_x, c.chunk_z))
        .collect();

    // Shrinking: the edge row goes. Everything else keeps its heights.
    if let Some((dropped_edge, idx)) = plan.dropped {
        for (entity, cx, cz) in &chunks {
            let on_dropped_row = match dropped_edge {
                GridEdge::MinX | GridEdge::MaxX => *cx == idx,
                GridEdge::MinZ | GridEdge::MaxZ => *cz == idx,
            };
            if on_dropped_row {
                if let Ok(e) = world.get_entity_mut(*entity) {
                    e.despawn();
                }
            }
        }
    }

    // Re-index the survivors. `saturating_add_signed` is belt-and-braces: the
    // dropped row is already gone, so nothing here can underflow.
    if plan.shift_x != 0 || plan.shift_z != 0 {
        for (entity, ..) in &chunks {
            let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) else {
                continue; // just despawned
            };
            chunk.chunk_x = chunk.chunk_x.saturating_add_signed(plan.shift_x);
            chunk.chunk_z = chunk.chunk_z.saturating_add_signed(plan.shift_z);
        }
    }

    // Compensate the parent so the sculpted terrain doesn't slide. The offset is
    // in the terrain's own local space, but `translation` is in its *parent's*,
    // so it has to go through the terrain's rotation and scale — otherwise a
    // rotated terrain jumps sideways instead of staying put.
    if let Some(mut transform) = world.get_mut::<Transform>(terrain) {
        let local = plan.local_offset * transform.scale;
        let delta = transform.rotation * local;
        transform.translation += delta;
    }

    // Last: the dimensions. This is the write `terrain_data_changed_system`
    // reacts to, and by now everything it reads is already consistent.
    if let Some(mut data) = world.get_mut::<TerrainData>(terrain) {
        data.chunks_x = plan.chunks_x;
        data.chunks_z = plan.chunks_z;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(chunks_x: u32, chunks_z: u32) -> TerrainData {
        TerrainData {
            chunks_x,
            chunks_z,
            chunk_size: 64.0,
            ..TerrainData::default()
        }
    }

    /// The invariant the whole module exists for: after an edge resize, a chunk
    /// that survived must sit at the same world position it did before.
    fn survivor_moved(d: &TerrainData, plan: &GridResize, cx: u32, cz: u32) -> Vec3 {
        let before = d.chunk_world_origin(cx, cz);
        let after_data = TerrainData {
            chunks_x: plan.chunks_x,
            chunks_z: plan.chunks_z,
            ..d.clone()
        };
        let after = after_data.chunk_world_origin(
            cx.saturating_add_signed(plan.shift_x),
            cz.saturating_add_signed(plan.shift_z),
        );
        // The parent moves by `local_offset`, so the chunk's final world
        // position is its local origin plus that.
        after + plan.local_offset - before
    }

    #[test]
    fn growing_any_edge_leaves_existing_chunks_where_they_were() {
        let d = data(3, 2);
        for &edge in GridEdge::all() {
            let plan = plan_grid_resize(&d, edge, 1).expect("grow is legal");
            for cz in 0..d.chunks_z {
                for cx in 0..d.chunks_x {
                    let moved = survivor_moved(&d, &plan, cx, cz);
                    assert!(
                        moved.length() < 1e-4,
                        "{edge:?}: chunk ({cx},{cz}) moved by {moved:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn shrinking_any_edge_leaves_surviving_chunks_where_they_were() {
        let d = data(3, 3);
        for &edge in GridEdge::all() {
            let plan = plan_grid_resize(&d, edge, -1).expect("shrink is legal");
            let (dropped_edge, idx) = plan.dropped.expect("shrink drops a row");
            assert_eq!(dropped_edge, edge);
            for cz in 0..d.chunks_z {
                for cx in 0..d.chunks_x {
                    let on_dropped = match edge {
                        GridEdge::MinX | GridEdge::MaxX => cx == idx,
                        GridEdge::MinZ | GridEdge::MaxZ => cz == idx,
                    };
                    if on_dropped {
                        continue;
                    }
                    let moved = survivor_moved(&d, &plan, cx, cz);
                    assert!(
                        moved.length() < 1e-4,
                        "{edge:?}: chunk ({cx},{cz}) moved by {moved:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn only_min_edges_reindex() {
        let d = data(2, 2);
        let grow_min = plan_grid_resize(&d, GridEdge::MinX, 1).unwrap();
        assert_eq!((grow_min.shift_x, grow_min.shift_z), (1, 0));
        let grow_max = plan_grid_resize(&d, GridEdge::MaxX, 1).unwrap();
        assert_eq!((grow_max.shift_x, grow_max.shift_z), (0, 0));
        let shrink_min = plan_grid_resize(&d, GridEdge::MinZ, -1).unwrap();
        assert_eq!((shrink_min.shift_x, shrink_min.shift_z), (0, -1));
        let shrink_max = plan_grid_resize(&d, GridEdge::MaxZ, -1).unwrap();
        assert_eq!((shrink_max.shift_x, shrink_max.shift_z), (0, 0));
    }

    #[test]
    fn resize_touches_only_its_own_axis() {
        let d = data(4, 6);
        let x = plan_grid_resize(&d, GridEdge::MaxX, 1).unwrap();
        assert_eq!((x.chunks_x, x.chunks_z), (5, 6));
        assert_eq!(x.local_offset.z, 0.0);
        let z = plan_grid_resize(&d, GridEdge::MinZ, 1).unwrap();
        assert_eq!((z.chunks_x, z.chunks_z), (4, 7));
        assert_eq!(z.local_offset.x, 0.0);
    }

    #[test]
    fn compensation_is_half_a_chunk_and_follows_chunk_size() {
        let mut d = data(2, 2);
        d.chunk_size = 100.0;
        let plan = plan_grid_resize(&d, GridEdge::MinX, 1).unwrap();
        assert_eq!(plan.local_offset, Vec3::new(-50.0, 0.0, 0.0));
        let plan = plan_grid_resize(&d, GridEdge::MaxX, 1).unwrap();
        assert_eq!(plan.local_offset, Vec3::new(50.0, 0.0, 0.0));
    }

    #[test]
    fn the_last_chunk_cannot_be_removed() {
        let d = data(1, 1);
        for &edge in GridEdge::all() {
            assert!(plan_grid_resize(&d, edge, -1).is_none(), "{edge:?}");
        }
    }

    #[test]
    fn growth_stops_at_the_axis_cap() {
        let d = data(MAX_CHUNKS_PER_AXIS, 1);
        assert!(plan_grid_resize(&d, GridEdge::MaxX, 1).is_none());
        // The capped axis doesn't block the other one.
        assert!(plan_grid_resize(&d, GridEdge::MaxZ, 1).is_some());
        // Shrinking a capped axis is still fine.
        assert!(plan_grid_resize(&d, GridEdge::MaxX, -1).is_some());
    }

    #[test]
    fn cost_counts_every_chunk() {
        let c = estimate_cost(4, 3, 129);
        assert_eq!(c.chunks, 12);
        assert_eq!(c.vertices, 12 * 129 * 129);
        assert_eq!(c.triangles, 12 * 128 * 128 * 2);
        assert!(c.bytes > c.vertices * 32);
    }

    #[test]
    fn cost_grows_with_the_square_of_resolution() {
        let low = estimate_cost(1, 1, 65);
        let high = estimate_cost(1, 1, 129);
        // 129² / 65² ≈ 3.94 — a single resolution step nearly quadruples the
        // cost, which is exactly why the overlay shows it before applying.
        let ratio = high.vertices as f64 / low.vertices as f64;
        assert!((3.8..4.0).contains(&ratio), "ratio was {ratio}");
    }

    /// The case the warning exists for: the old slider's top setting.
    #[test]
    fn a_full_grid_at_top_resolution_trips_the_warning() {
        assert!(estimate_cost(8, 8, 257).vertices > COST_WARN_VERTICES);
        // …while an ordinary working terrain doesn't.
        assert!(estimate_cost(4, 4, 129).vertices < COST_WARN_VERTICES);
    }

    #[test]
    fn a_degenerate_resolution_doesnt_underflow() {
        let c = estimate_cost(1, 1, 0);
        assert_eq!(c.vertices, 0);
        assert_eq!(c.triangles, 0);
    }

    #[test]
    fn only_single_step_resizes_are_planned() {
        let d = data(4, 4);
        assert!(plan_grid_resize(&d, GridEdge::MaxX, 0).is_none());
        assert!(plan_grid_resize(&d, GridEdge::MaxX, 2).is_none());
        assert!(plan_grid_resize(&d, GridEdge::MaxX, -2).is_none());
    }
}
