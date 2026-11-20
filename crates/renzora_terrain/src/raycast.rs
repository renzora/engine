//! Ray against terrain, answered from the heights rather than the meshes.
//!
//! The brushes used Bevy's mesh ray cast, which tests every triangle of every
//! mesh whose bounds the ray crosses. A terrain chunk is ~33k triangles, and a
//! cursor ray skimming a large terrain crosses the bounds of many chunks, so
//! merely having a terrain selected (which arms the sculpt brush, and with it a
//! hover cast every frame) cost a 512 m terrain over 20 ms a frame.
//!
//! A heightfield needs none of that. The ray is clipped to the terrain's box
//! and marched in half-cell steps; the first step that lands below the surface
//! is bisected back to the crossing. The cost is a few hundred height lookups
//! whatever the chunk count, and the surface sampled is the one drawn: each
//! cell is split along the same diagonal as [`crate::mesh::generate_chunk_mesh`]
//! splits it.

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Where a ray met a terrain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainRayHit {
    /// The terrain root.
    pub terrain: Entity,
    /// The chunk the hit is on.
    pub chunk: Entity,
    /// World-space point on the surface.
    pub point: Vec3,
    /// World-space distance from the ray's origin.
    pub distance: f32,
    /// Position within the chunk, 0..1 on each axis.
    pub chunk_uv: Vec2,
}

/// One terrain's heights, indexed by chunk for O(1) lookups.
pub struct Heightfield<'a> {
    terrain: &'a TerrainData,
    /// `cz * chunks_x + cx`; `None` where a chunk is missing.
    chunks: Vec<Option<(Entity, &'a TerrainChunkData)>>,
}

impl<'a> Heightfield<'a> {
    pub fn new(
        terrain: &'a TerrainData,
        chunks: impl IntoIterator<Item = (Entity, &'a TerrainChunkData)>,
    ) -> Self {
        let mut slots = vec![None; (terrain.chunks_x * terrain.chunks_z) as usize];
        for (entity, chunk) in chunks {
            if chunk.chunk_x < terrain.chunks_x && chunk.chunk_z < terrain.chunks_z {
                slots[(chunk.chunk_z * terrain.chunks_x + chunk.chunk_x) as usize] = Some((entity, chunk));
            }
        }
        Self { terrain, chunks: slots }
    }

    /// Which chunk terrain-local `(x, z)` falls in, and where within it in
    /// metres. A point on the far edge belongs to the last chunk.
    fn locate(&self, x: f32, z: f32) -> Option<(Entity, &'a TerrainChunkData, f32, f32)> {
        let t = self.terrain;
        let gx = x + t.total_width() / 2.0;
        let gz = z + t.total_depth() / 2.0;
        if !(0.0..=t.total_width()).contains(&gx) || !(0.0..=t.total_depth()).contains(&gz) {
            return None;
        }
        let cx = ((gx / t.chunk_size) as u32).min(t.chunks_x - 1);
        let cz = ((gz / t.chunk_size) as u32).min(t.chunks_z - 1);
        let (entity, chunk) = self.chunks[(cz * t.chunks_x + cx) as usize]?;
        Some((entity, chunk, gx - cx as f32 * t.chunk_size, gz - cz as f32 * t.chunk_size))
    }

    /// Surface height in terrain-local metres at terrain-local `(x, z)`, on
    /// the drawn triangles.
    pub fn height(&self, x: f32, z: f32) -> Option<f32> {
        let (_, chunk, lx, lz) = self.locate(x, z)?;
        let t = self.terrain;
        let res = t.chunk_resolution;
        let last = (res - 2) as f32;
        let fx = (lx / t.vertex_spacing()).clamp(0.0, last + 1.0);
        let fz = (lz / t.vertex_spacing()).clamp(0.0, last + 1.0);
        let (ix, iz) = (fx.floor().min(last), fz.floor().min(last));
        let (u, v) = (fx - ix, fz - iz);
        let (ix, iz) = (ix as u32, iz as u32);
        let h = |x: u32, z: u32| chunk.get_height(x, z, res);
        let (tl, tr, bl, br) = (h(ix, iz), h(ix + 1, iz), h(ix, iz + 1), h(ix + 1, iz + 1));
        // The mesh's triangles are (tl, bl, tr) and (tr, bl, br): the diagonal
        // runs from tr to bl, where u + v = 1.
        let normalized = if u + v <= 1.0 {
            tl + u * (tr - tl) + v * (bl - tl)
        } else {
            br + (1.0 - u) * (bl - br) + (1.0 - v) * (tr - br)
        };
        Some(t.min_height + normalized * t.height_range())
    }

    /// The first crossing of `ray` (terrain-local) with the surface, as the
    /// ray parameter, within `0..max_t`.
    fn march(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let t = self.terrain;
        let lo = Vec3::new(-t.total_width() / 2.0, t.min_height, -t.total_depth() / 2.0);
        let hi = Vec3::new(t.total_width() / 2.0, t.max_height, t.total_depth() / 2.0);
        let (enter, exit) = clip_to_box(origin, dir, lo, hi)?;
        let (enter, exit) = (enter.max(0.0), exit.min(max_t));
        if enter > exit {
            return None;
        }
        let above = |s: f32| {
            let p = origin + dir * s;
            self.height(p.x, p.z).map(|h| p.y - h)
        };
        // Half a cell along the ground; a steep ray needs fewer steps, so the
        // step is measured across the horizontal part of the direction.
        let horizontal = dir.xz().length().max(1e-4);
        let step = (t.vertex_spacing() * 0.5 / horizontal).min(exit - enter).max(1e-3);
        let mut prev_s = enter;
        let mut prev = above(enter);
        if prev.is_some_and(|d| d <= 0.0) {
            return Some(enter);
        }
        let mut s = enter;
        while s < exit {
            s = (s + step).min(exit);
            let now = above(s);
            if let (Some(a), Some(b)) = (prev, now) {
                if a > 0.0 && b <= 0.0 {
                    let (mut lo_s, mut hi_s) = (prev_s, s);
                    for _ in 0..20 {
                        let mid = (lo_s + hi_s) * 0.5;
                        if above(mid).is_some_and(|d| d > 0.0) {
                            lo_s = mid;
                        } else {
                            hi_s = mid;
                        }
                    }
                    return Some(hi_s);
                }
            }
            prev_s = s;
            prev = now;
        }
        None
    }
}

/// The ray's entry and exit parameters through an axis-aligned box.
fn clip_to_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<(f32, f32)> {
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    for axis in 0..3 {
        let (o, d) = (origin[axis], dir[axis]);
        if d.abs() < 1e-9 {
            if o < lo[axis] || o > hi[axis] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[axis] - o) / d, (hi[axis] - o) / d);
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
    }
    (enter <= exit).then_some((enter, exit))
}

/// The nearest hit of `ray` on any of `terrains`, each made of the `chunks`
/// that name it as their terrain.
pub fn raycast_terrains<'a>(
    ray: Ray3d,
    terrains: impl IntoIterator<Item = (Entity, &'a TerrainData, &'a GlobalTransform)>,
    chunks: impl IntoIterator<Item = (Entity, &'a TerrainChunkData, &'a TerrainChunkOf)> + Clone,
) -> Option<TerrainRayHit> {
    let mut best: Option<TerrainRayHit> = None;
    for (terrain_entity, terrain, transform) in terrains {
        if terrain.chunks_x == 0 || terrain.chunks_z == 0 || terrain.chunk_resolution < 2 {
            continue;
        }
        let field = Heightfield::new(
            terrain,
            chunks
                .clone()
                .into_iter()
                .filter(|(_, _, of)| of.0 == terrain_entity)
                .map(|(e, c, _)| (e, c)),
        );
        let to_local = transform.affine().inverse();
        let origin = to_local.transform_point3(ray.origin);
        let dir = to_local.transform_vector3(*ray.direction);
        // World distance is measured after the hit; a far limit in local
        // units only bounds the march.
        let Some(s) = field.march(origin, dir, f32::MAX) else { continue };
        let local = origin + dir * s;
        let point = transform.transform_point(local);
        let distance = point.distance(ray.origin);
        if best.is_some_and(|b| b.distance <= distance) {
            continue;
        }
        let Some((chunk, _, lx, lz)) = field.locate(local.x, local.z) else { continue };
        best = Some(TerrainRayHit {
            terrain: terrain_entity,
            chunk,
            point,
            distance,
            chunk_uv: Vec2::new(lx, lz) / terrain.chunk_size,
        });
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData {
            chunks_x: 2,
            chunks_z: 2,
            chunk_size: 8.0,
            chunk_resolution: 9,
            min_height: 0.0,
            max_height: 10.0,
            ..default()
        }
    }

    /// Chunks whose heights rise along x: normalized `x_metres / 16`.
    fn ramp(t: &TerrainData) -> Vec<TerrainChunkData> {
        let mut out = Vec::new();
        for cz in 0..t.chunks_z {
            for cx in 0..t.chunks_x {
                let mut chunk = TerrainChunkData::new(cx, cz, t.chunk_resolution, 0.0);
                for z in 0..t.chunk_resolution {
                    for x in 0..t.chunk_resolution {
                        let metres = cx as f32 * t.chunk_size + x as f32 * t.vertex_spacing();
                        chunk.heights[(z * t.chunk_resolution + x) as usize] = metres / 16.0;
                    }
                }
                out.push(chunk);
            }
        }
        out
    }

    fn hit(t: &TerrainData, chunks: &[TerrainChunkData], ray: Ray3d) -> Option<TerrainRayHit> {
        let root = Entity::from_raw_u32(1).unwrap();
        let ids: Vec<Entity> = (0..chunks.len()).map(|i| Entity::from_raw_u32(10 + i as u32).unwrap()).collect();
        let of = TerrainChunkOf(root);
        let transform = GlobalTransform::default();
        raycast_terrains(
            ray,
            [(root, t, &transform)],
            ids.iter().zip(chunks).map(|(e, c)| (*e, c, &of)).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn a_ray_straight_down_lands_on_the_surface_height() {
        let t = terrain();
        let chunks = ramp(&t);
        // Terrain-local x = 4 is 12 m from the grid's left edge: 12/16 * 10 m.
        let h = hit(&t, &chunks, Ray3d::new(Vec3::new(4.0, 50.0, 1.0), Dir3::NEG_Y)).unwrap();
        assert!((h.point.y - 7.5).abs() < 1e-3, "{:?}", h.point);
        assert!((h.distance - 42.5).abs() < 1e-3);
        // x = 4 local is 4 m into the second chunk of the row.
        assert!((h.chunk_uv.x - 0.5).abs() < 1e-4);
    }

    #[test]
    fn a_grazing_ray_meets_the_slope_where_it_rises_to_meet_it() {
        let t = terrain();
        let chunks = ramp(&t);
        // Level ray at y = 5 hits where the ramp reaches 5 m: 8 m from the
        // left edge, which is terrain-local x = 0.
        let h = hit(&t, &chunks, Ray3d::new(Vec3::new(-20.0, 5.0, 2.0), Dir3::X)).unwrap();
        assert!(h.point.x.abs() < 1e-3, "{:?}", h.point);
    }

    #[test]
    fn a_ray_that_misses_the_terrain_misses() {
        let t = terrain();
        let chunks = ramp(&t);
        assert!(hit(&t, &chunks, Ray3d::new(Vec3::new(0.0, 50.0, 0.0), Dir3::Y)).is_none());
        assert!(hit(&t, &chunks, Ray3d::new(Vec3::new(100.0, 50.0, 0.0), Dir3::NEG_Y)).is_none());
    }

    #[test]
    fn heights_follow_the_drawn_triangles() {
        let t = terrain();
        let mut chunks = ramp(&t);
        // One raised vertex: the cell's two triangles slope differently, and
        // the sample must follow the one that contains the point.
        let res = t.chunk_resolution;
        chunks[0].heights.iter_mut().for_each(|h| *h = 0.0);
        chunks[0].heights[(res + 1) as usize] = 1.0; // (1, 1): bottom-right of cell (0, 0)
        let field = Heightfield::new(&t, chunks.iter().enumerate().map(|(i, c)| (Entity::from_raw_u32(i as u32 + 1).unwrap(), c)));
        let s = t.vertex_spacing();
        let (x0, z0) = (-t.total_width() / 2.0, -t.total_depth() / 2.0);
        // Below the tr-bl diagonal the raised corner is not part of the
        // triangle: flat.
        assert!(field.height(x0 + 0.25 * s, z0 + 0.25 * s).unwrap().abs() < 1e-5);
        // Past it, the height rises toward br.
        let h = field.height(x0 + 0.75 * s, z0 + 0.75 * s).unwrap();
        assert!((h - 0.5 * t.height_range()).abs() < 1e-4, "{h}");
    }
}
