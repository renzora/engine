//! Water meshes: a bounded grid, and a camera-centred clipmap.
//!
//! Both are flat in the asset — every wave is vertex displacement sampled from
//! the cascade maps in `water.wgsl`. What the mesh decides is only *where the
//! triangles are*.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
// Bevy's hasher, not std's: interning runs once per clipmap vertex and a
// dense clipmap has a few hundred thousand of them, where SipHash is
// measurably slower than foldhash.
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Generate a flat subdivided XZ plane centered at origin.
pub fn generate_water_mesh(size: f32, subdivisions: u32) -> Mesh {
    let subdivisions = subdivisions.max(1);
    let verts_per_edge = subdivisions + 1;
    let total_verts = (verts_per_edge * verts_per_edge) as usize;
    let total_indices = (subdivisions * subdivisions * 6) as usize;

    let mut positions = Vec::with_capacity(total_verts);
    let mut normals = Vec::with_capacity(total_verts);
    let mut uvs = Vec::with_capacity(total_verts);
    let mut indices = Vec::with_capacity(total_indices);

    let half = size * 0.5;

    for z in 0..verts_per_edge {
        for x in 0..verts_per_edge {
            let fx = x as f32 / subdivisions as f32;
            let fz = z as f32 / subdivisions as f32;

            positions.push([-half + fx * size, 0.0, -half + fz * size]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([fx, fz]);
        }
    }

    for z in 0..subdivisions {
        for x in 0..subdivisions {
            let tl = z * verts_per_edge + x;
            let tr = tl + 1;
            let bl = tl + verts_per_edge;
            let br = bl + 1;

            indices.push(tl);
            indices.push(bl);
            indices.push(tr);

            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    build_mesh(positions, normals, uvs, indices)
}

/// Generate a clipmap: a dense centre block surrounded by `rings` square
/// annuli, each with twice the quad size of the one inside it.
///
/// Level 0 is `resolution²` quads of `quad_size`, so it spans
/// `resolution * quad_size` metres. Ring *k* is another `resolution²` grid of
/// quads sized `quad_size * 2^k`, with its central quarter removed — that hole
/// is exactly the extent of everything inside it. Half-extent therefore grows
/// as `resolution * quad_size / 2 * 2^rings`, while the triangle count grows
/// only linearly with `rings`.
///
/// Seams are watertight rather than merely close: every vertex is snapped to
/// the `quad_size` lattice and shared through one lookup table, and each ring's
/// inner boundary quads are triangulated with their inner edge **split in two**
/// so they meet the finer level's twice-as-dense edge vertex-for-vertex. Skip
/// that split and each seam T-junction opens into a visible crack the moment
/// the surface is displaced.
pub fn generate_clipmap_mesh(rings: u32, resolution: u32, quad_size: f32) -> Mesh {
    // The hole is the central quarter, so a level needs an even cell count; a
    // multiple of 4 also keeps the hole itself even for the next ring out.
    let res = resolution.max(4) & !3;
    let quad_size = quad_size.max(0.01);
    let rings = rings.min(16);

    let mut builder = ClipmapBuilder::new(quad_size, res, rings);

    // Level 0: solid centre block.
    builder.add_level(res, quad_size, false);

    // Rings: same cell count, double the quad size, central quarter removed.
    for k in 1..=rings {
        builder.add_level(res, quad_size * (1u32 << k) as f32, true);
    }

    builder.finish()
}

struct ClipmapBuilder {
    /// Lattice step used to key vertices. Every position in the clipmap is a
    /// multiple of the finest quad size, including the split-edge midpoints.
    lattice: f32,
    lookup: HashMap<(i64, i64), u32>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl ClipmapBuilder {
    fn new(lattice: f32, res: u32, rings: u32) -> Self {
        // Centre block plus one ring per level, each ring being the level's
        // grid minus its central quarter.
        let cells = (res * res) as usize + (rings as usize) * (res * res * 3 / 4) as usize;
        Self {
            lattice,
            lookup: HashMap::with_capacity_and_hasher(cells, Default::default()),
            positions: Vec::with_capacity(cells),
            normals: Vec::with_capacity(cells),
            uvs: Vec::with_capacity(cells),
            indices: Vec::with_capacity(cells * 6),
        }
    }

    /// Interning a vertex by lattice coordinate is what makes neighbouring
    /// levels share their boundary vertices instead of merely coinciding.
    fn vertex(&mut self, x: f32, z: f32) -> u32 {
        let key = (
            (x / self.lattice).round() as i64,
            (z / self.lattice).round() as i64,
        );
        if let Some(&index) = self.lookup.get(&key) {
            return index;
        }
        let index = self.positions.len() as u32;
        self.positions.push([x, 0.0, z]);
        self.normals.push([0.0, 1.0, 0.0]);
        self.uvs.push([x, z]);
        self.lookup.insert(key, index);
        index
    }

    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.push(a);
        self.indices.push(b);
        self.indices.push(c);
    }

    fn add_level(&mut self, res: u32, quad: f32, hollow: bool) {
        let half = res as f32 * quad * 0.5;
        // Central quarter, in cell indices.
        let hole_lo = res / 4;
        let hole_hi = res - res / 4;

        for cz in 0..res {
            for cx in 0..res {
                let in_hole =
                    hollow && cx >= hole_lo && cx < hole_hi && cz >= hole_lo && cz < hole_hi;
                if in_hole {
                    continue;
                }

                let x0 = -half + cx as f32 * quad;
                let x1 = x0 + quad;
                let z0 = -half + cz as f32 * quad;
                let z1 = z0 + quad;

                // Which edge (if any) faces the hole, i.e. the finer level.
                let (split_left, split_right, split_top, split_bottom) = if hollow {
                    let row_in_hole = cz >= hole_lo && cz < hole_hi;
                    let col_in_hole = cx >= hole_lo && cx < hole_hi;
                    (
                        row_in_hole && cx == hole_hi,  // hole is to -X
                        row_in_hole && cx + 1 == hole_lo, // hole is to +X
                        col_in_hole && cz + 1 == hole_lo, // hole is to +Z
                        col_in_hole && cz == hole_hi,  // hole is to -Z
                    )
                } else {
                    (false, false, false, false)
                };

                let a = self.vertex(x0, z0); // -X -Z
                let b = self.vertex(x1, z0); // +X -Z
                let c = self.vertex(x1, z1); // +X +Z
                let d = self.vertex(x0, z1); // -X +Z

                if split_left {
                    let m = self.vertex(x0, (z0 + z1) * 0.5);
                    self.tri(a, m, b);
                    self.tri(m, c, b);
                    self.tri(m, d, c);
                } else if split_right {
                    let m = self.vertex(x1, (z0 + z1) * 0.5);
                    self.tri(a, d, m);
                    self.tri(d, c, m);
                    self.tri(a, m, b);
                } else if split_top {
                    let m = self.vertex((x0 + x1) * 0.5, z1);
                    self.tri(a, m, b);
                    self.tri(a, d, m);
                    self.tri(m, c, b);
                } else if split_bottom {
                    let m = self.vertex((x0 + x1) * 0.5, z0);
                    self.tri(a, d, m);
                    self.tri(m, d, c);
                    self.tri(m, c, b);
                } else {
                    self.tri(a, d, b);
                    self.tri(b, d, c);
                }
            }
        }
    }

    fn finish(self) -> Mesh {
        build_mesh(self.positions, self.normals, self.uvs, self.indices)
    }
}

fn build_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_count(mesh: &Mesh) -> usize {
        match mesh.indices() {
            Some(Indices::U32(i)) => i.len(),
            Some(Indices::U16(i)) => i.len(),
            None => 0,
        }
    }

    #[test]
    fn grid_mesh_has_expected_topology() {
        let mesh = generate_water_mesh(10.0, 4);
        assert_eq!(mesh.count_vertices(), 25);
        assert_eq!(index_count(&mesh), 4 * 4 * 6);
    }

    #[test]
    fn clipmap_extent_doubles_per_ring() {
        // Half-extent must be res*quad/2 * 2^rings — that relationship is what
        // lets a handful of rings reach the horizon.
        let res = 8;
        let quad = 1.0;
        for rings in 0..4 {
            let mesh = generate_clipmap_mesh(rings, res, quad);
            let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
            let max_x = positions
                .as_float3()
                .unwrap()
                .iter()
                .fold(0.0f32, |acc, p| acc.max(p[0].abs()));
            let expected = res as f32 * quad * 0.5 * (1u32 << rings) as f32;
            assert!(
                (max_x - expected).abs() < 1e-3,
                "rings={rings}: half-extent {max_x} != {expected}"
            );
        }
    }

    #[test]
    fn clipmap_seams_are_watertight() {
        // Every interior edge must be shared by exactly two triangles. A
        // T-junction at a ring seam shows up here as an edge used once, and
        // on screen as a crack the moment waves displace the surface.
        let mesh = generate_clipmap_mesh(2, 8, 1.0);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("expected u32 indices");
        };
        let mut edges: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for tri in indices.chunks_exact(3) {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let key = if a < b { (a, b) } else { (b, a) };
                *edges.entry(key).or_insert(0) += 1;
            }
        }

        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let positions = positions.as_float3().unwrap();
        let half = 8.0 * 1.0 * 0.5 * 4.0;
        for ((a, b), count) in edges {
            let pa = positions[a as usize];
            let pb = positions[b as usize];
            // Edges on the outermost border are legitimately used once.
            let on_border = |p: &[f32; 3]| {
                (p[0].abs() - half).abs() < 1e-3 || (p[2].abs() - half).abs() < 1e-3
            };
            if on_border(&pa) && on_border(&pb) {
                continue;
            }
            assert_eq!(count, 2, "edge {a}-{b} used {count} times (not watertight)");
        }
    }

    #[test]
    fn default_clipmap_builds_in_reasonable_time() {
        // The shipped defaults are dense (matching the reference's ~600k
        // triangles), and this runs on the main thread whenever a clipmap
        // parameter changes — so a regression that makes interning slow would
        // show up as an editor hitch, not a test failure. Pin it.
        let start = std::time::Instant::now();
        let mesh = generate_clipmap_mesh(5, 256, 0.25);
        let elapsed = start.elapsed();
        assert!(mesh.count_vertices() > 200_000, "unexpectedly sparse");
        assert!(
            elapsed.as_millis() < 2000,
            "clipmap build took {elapsed:?}"
        );
    }

    #[test]
    fn clipmap_has_no_degenerate_triangles() {
        let mesh = generate_clipmap_mesh(2, 8, 1.0);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("expected u32 indices");
        };
        for tri in indices.chunks_exact(3) {
            assert!(
                tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2],
                "degenerate triangle {tri:?}"
            );
        }
    }
}
