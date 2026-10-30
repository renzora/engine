//! Terrain level of detail: coarser versions of each chunk at a distance.
//!
//! Every chunk keeps its full-detail mesh: the collider is built from it, and
//! it is what is drawn up close. Beside it, as children, sit up to
//! `TerrainData::lod_levels` coarser meshes, each with half the vertices per
//! side of the one before. Bevy's [`VisibilityRange`] picks one per chunk by
//! camera distance, so the switch costs nothing on the CPU and happens per
//! view (a second viewport or a shadow view gets its own choice).
//!
//! The distances double with each level, starting at three chunk widths,
//! times `TerrainData::lod_distance`. A level is visible in a band of
//! distance, and neighbouring bands share their boundary (see `CROSSFADE` for
//! why they do not overlap and dither), so a chunk is never drawn twice or
//! not at all.
//!
//! Seams: a coarse chunk keeps its border at full detail, so neighbours at any
//! two levels share every edge vertex and no slit can open between them (see
//! [`crate::mesh::generate_chunk_lod_mesh`]).
//!
//! The level meshes follow the chunk: they are rebuilt when its heights
//! change, wear the chunk's material, and go when the chunk's mesh goes
//! (streamed out, or the chunk despawned).

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use renzora::core::MaterialRef;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::material::TerrainCheckerboardMaterial;
use crate::mesh::{generate_chunk_lod_mesh_beside, ChunkNeighbours};
use std::collections::HashMap;
#[cfg(test)]
use crate::mesh::generate_chunk_lod_mesh;
use crate::surface::TerrainSurfaceMaterial;
use renzora_shader::material::GraphMaterial;

/// A coarser mesh of the chunk it is a child of. `level` 1 has half the
/// chunk's vertices per side, 2 a quarter, and so on.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkLodLevel {
    pub level: u32,
}

/// The level meshes a chunk has, nearest first, and the settings they were
/// made for.
#[derive(Component, Debug)]
pub struct ChunkLods {
    pub levels: Vec<(Entity, Handle<Mesh>)>,
    made_for: LodKey,
}

/// What a chunk's levels depend on besides its heights.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LodKey {
    levels: u32,
    distance: f32,
    chunk_size: f32,
    resolution: u32,
}

impl LodKey {
    fn of(terrain: &TerrainData) -> Self {
        Self {
            levels: usable_levels(terrain),
            distance: terrain.lod_distance,
            chunk_size: terrain.chunk_size,
            resolution: terrain.chunk_resolution,
        }
    }
}

/// How many coarser levels the chunk grid can have: each halves the
/// vertices per side, which needs the cell count to divide evenly, and the
/// coarsest keeps at least two cells per side.
pub fn usable_levels(terrain: &TerrainData) -> u32 {
    let cells = terrain.chunk_resolution.saturating_sub(1);
    let mut levels = 0;
    while levels < terrain.lod_levels {
        let step = 1u32 << (levels + 1);
        if !cells.is_multiple_of(step) || cells / step < 2 {
            break;
        }
        levels += 1;
    }
    levels
}

/// The camera distance at which `level` starts to be drawn; level 0 starts
/// at 0. Doubles per level from three chunk widths. It was one and a half,
/// which put the first switch about fifty metres from the camera, close
/// enough on eroded ground to watch every chunk change shape.
pub fn level_start(terrain: &TerrainData, level: u32) -> f32 {
    if level == 0 {
        return 0.0;
    }
    3.0 * terrain.chunk_size * terrain.lod_distance.max(0.05) * (1u32 << (level - 1)) as f32
}

/// The share of a switch distance over which two levels cross-fade, centred
/// on it. Zero: Bevy's cross-fade is a per-pixel dither that only temporal
/// anti-aliasing smooths, and without it a chunk resting in the band showed a
/// fixed halftone pattern, which read worse than the pop it replaced. Kept as
/// a knob (and the surface shader keeps its dither call) for a view that
/// renders with TAA.
const CROSSFADE: f32 = 0.0;

/// The distance band `level` is drawn in, out of `levels` coarser ones.
/// Measured to the centre of the mesh's bounds, so every level of a chunk
/// measures from the same point.
///
/// With a non-zero [`CROSSFADE`], neighbouring levels overlap by a band
/// around each switch distance, where Bevy dithers one out as the other comes
/// in (the surface shader does the dither).
pub fn level_range(terrain: &TerrainData, level: u32, levels: u32) -> VisibilityRange {
    let band = |d: f32| {
        let half = d * CROSSFADE * 0.5;
        (d - half)..(d + half)
    };
    let start = level_start(terrain, level);
    let start_margin = if level == 0 { 0.0..0.0 } else { band(start) };
    let end_margin = if level >= levels {
        f32::MAX..f32::MAX
    } else {
        band(level_start(terrain, level + 1))
    };
    VisibilityRange { start_margin, end_margin, use_aabb: true }
}

/// Give every meshed chunk its levels, and remake them when the settings
/// they depend on change.
pub fn ensure_chunk_lods(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrains: Query<&TerrainData>,
    chunks: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, Option<&ChunkLods>), With<Mesh3d>>,
    all: Query<(&TerrainChunkData, &TerrainChunkOf)>,
) {
    let mut grid = None;
    for (chunk_entity, chunk, of, lods) in &chunks {
        let Ok(terrain) = terrains.get(of.0) else { continue };
        let key = LodKey::of(terrain);
        if lods.is_some_and(|l| l.made_for == key) {
            continue;
        }
        if let Some(old) = lods {
            for (child, _) in &old.levels {
                commands.entity(*child).try_despawn();
            }
        }
        if key.levels == 0 {
            commands.entity(chunk_entity).remove::<(ChunkLods, VisibilityRange)>();
            continue;
        }
        // Heights not composed yet (a chunk straight off a scene load): the
        // mesh update rebuilds the levels once they are.
        let grid = grid.get_or_insert_with(|| chunk_grid(&all));
        let neighbours = ChunkNeighbours::around(chunk.chunk_x, chunk.chunk_z, |x, z| grid.get(&(of.0, x, z)).copied());
        let mut levels = Vec::with_capacity(key.levels as usize);
        for level in 1..=key.levels {
            let mesh = meshes.add(generate_chunk_lod_mesh_beside(terrain, chunk, 1 << level, neighbours));
            let child = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    level_range(terrain, level, key.levels),
                    ChunkLodLevel { level },
                    // Generated, rebuilt from the chunk, never saved: kept out
                    // of the hierarchy, and out of the shipped game's sweep of
                    // unnamed entities.
                    renzora::HideInHierarchy,
                    ChildOf(chunk_entity),
                ))
                .id();
            levels.push((child, mesh));
        }
        commands
            .entity(chunk_entity)
            .insert((level_range(terrain, 0, key.levels), ChunkLods { levels, made_for: key }));
    }
}

/// A chunk that lost its mesh (streamed out) loses its levels with it; they
/// come back with the mesh.
pub fn drop_orphaned_lods(
    mut commands: Commands,
    chunks: Query<(Entity, &ChunkLods), Without<Mesh3d>>,
) {
    for (chunk, lods) in &chunks {
        for (child, _) in &lods.levels {
            commands.entity(*child).try_despawn();
        }
        commands.entity(chunk).remove::<(ChunkLods, VisibilityRange)>();
    }
}

/// Rebuild the level meshes of chunks whose heights changed. Runs on the
/// same flag as the full mesh rebuild, just before it clears it.
pub fn rebuild_lod_meshes(
    mut meshes: ResMut<Assets<Mesh>>,
    terrains: Query<&TerrainData>,
    chunks: Query<(&TerrainChunkData, &TerrainChunkOf, &ChunkLods)>,
    all: Query<(&TerrainChunkData, &TerrainChunkOf)>,
) {
    let mut grid = None;
    for (chunk, of, lods) in &chunks {
        if !chunk.mesh_stale {
            continue;
        }
        let Ok(terrain) = terrains.get(of.0) else { continue };
        let grid = grid.get_or_insert_with(|| chunk_grid(&all));
        let neighbours = ChunkNeighbours::around(chunk.chunk_x, chunk.chunk_z, |x, z| grid.get(&(of.0, x, z)).copied());
        for (i, (_, handle)) in lods.levels.iter().enumerate() {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                *mesh = generate_chunk_lod_mesh_beside(terrain, chunk, 1 << (i + 1), neighbours);
            }
        }
    }
}

/// Every chunk by terrain and grid cell, for [`ChunkNeighbours::around`].
fn chunk_grid<'a>(
    all: &'a Query<(&TerrainChunkData, &TerrainChunkOf)>,
) -> HashMap<(Entity, u32, u32), &'a TerrainChunkData> {
    all.iter().map(|(chunk, of)| ((of.0, chunk.chunk_x, chunk.chunk_z), chunk)).collect()
}

/// Dress each level like its chunk: the same surface, landscape or
/// checkerboard material, or the same project material.
///
/// The landscape material (a `GraphMaterial`, or a `StandardMaterial` when the
/// terrain's material is a plain one) is copied as a handle only for a chunk
/// without a `MaterialRef`: a chunk with one has it resolved for it, and so
/// does each level, which carries the same reference.
#[allow(clippy::type_complexity)]
pub fn sync_lod_materials(
    mut commands: Commands,
    chunks: Query<(
        &ChunkLods,
        Option<&MeshMaterial3d<TerrainSurfaceMaterial>>,
        Option<&MeshMaterial3d<TerrainCheckerboardMaterial>>,
        Option<&MaterialRef>,
        Option<&MeshMaterial3d<GraphMaterial>>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    levels: Query<(
        Option<&MeshMaterial3d<TerrainSurfaceMaterial>>,
        Option<&MeshMaterial3d<TerrainCheckerboardMaterial>>,
        Option<&MaterialRef>,
        Option<&MeshMaterial3d<GraphMaterial>>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    for (lods, surface, checker, material_ref, graph, standard) in &chunks {
        let (graph, standard) = if material_ref.is_some() { (None, None) } else { (graph, standard) };
        for (child, _) in &lods.levels {
            let Ok((c_surface, c_checker, c_ref, c_graph, c_standard)) = levels.get(*child) else { continue };
            let (c_graph, c_standard) = if c_ref.is_some() { (None, None) } else { (c_graph, c_standard) };
            let same = c_surface.map(|m| m.id()) == surface.map(|m| m.id())
                && c_checker.map(|m| m.id()) == checker.map(|m| m.id())
                && c_ref.map(|r| &r.0) == material_ref.map(|r| &r.0)
                && c_graph.map(|m| m.id()) == graph.map(|m| m.id())
                && c_standard.map(|m| m.id()) == standard.map(|m| m.id());
            if same {
                continue;
            }
            let mut e = commands.entity(*child);
            e.remove::<(
                MeshMaterial3d<TerrainSurfaceMaterial>,
                MeshMaterial3d<TerrainCheckerboardMaterial>,
                MaterialRef,
                MeshMaterial3d<GraphMaterial>,
                MeshMaterial3d<StandardMaterial>,
                renzora::core::MaterialResolved,
            )>();
            if let Some(m) = surface {
                e.insert(MeshMaterial3d(m.0.clone()));
            }
            if let Some(m) = checker {
                e.insert(MeshMaterial3d(m.0.clone()));
            }
            if let Some(m) = graph {
                e.insert(MeshMaterial3d(m.0.clone()));
            }
            if let Some(m) = standard {
                e.insert(MeshMaterial3d(m.0.clone()));
            }
            if let Some(r) = material_ref {
                e.insert(r.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::{Indices, VertexAttributeValues};

    fn terrain(levels: u32) -> TerrainData {
        TerrainData { chunks_x: 2, chunks_z: 2, chunk_size: 64.0, chunk_resolution: 129, lod_levels: levels, ..default() }
    }

    #[test]
    fn levels_stop_where_the_grid_stops_halving() {
        assert_eq!(usable_levels(&terrain(4)), 4);
        // 128 cells halve down to 2 per side at step 64: six levels at most.
        assert_eq!(usable_levels(&terrain(10)), 6);
        assert_eq!(usable_levels(&terrain(0)), 0);
        let odd = TerrainData { chunk_resolution: 100, lod_levels: 4, ..default() };
        assert_eq!(usable_levels(&odd), 0, "99 cells do not halve");
    }

    #[test]
    fn neighbouring_bands_share_their_boundary_and_the_last_never_ends() {
        let t = terrain(4);
        for level in 0..4 {
            let near = level_range(&t, level, 4);
            let far = level_range(&t, level + 1, 4);
            assert_eq!(near.end_margin, far.start_margin, "level {level}");
        }
        assert_eq!(level_range(&t, 4, 4).end_margin.start, f32::MAX);
        // Level 1 at three 64 m chunk widths.
        assert_eq!(level_range(&t, 1, 4).start_margin, 192.0..192.0);
        assert_eq!(level_start(&t, 2), 384.0);
        assert_eq!(level_range(&t, 0, 4).start_margin, 0.0..0.0);
    }

    #[test]
    fn a_coarse_level_has_a_quarter_of_the_vertices_and_a_full_detail_border() {
        let t = terrain(4);
        // An inner chunk: full detail has no walls at all.
        let chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        let inner = TerrainData { chunks_x: 3, chunks_z: 3, ..t.clone() };
        let interior = TerrainChunkData::new(1, 1, t.chunk_resolution, 0.5);
        let count = |m: &Mesh| match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.len(),
            _ => 0,
        };
        let full = generate_chunk_lod_mesh(&inner, &interior, 1);
        assert_eq!(count(&full), 129 * 129 + 4, "surface and floor only");
        let coarse = generate_chunk_lod_mesh(&inner, &interior, 2);
        // 65² surface, the 64 in-between border samples on each side, a
        // centre for each of the 252 ring cells, and 4 floor corners.
        assert_eq!(count(&coarse), 65 * 65 + 4 * 64 + 252 + 4);
        // Same footprint at any level.
        let max_x = |m: &Mesh| match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| p[0]).fold(f32::MIN, f32::max),
            _ => 0.0,
        };
        let (a, b) = (generate_chunk_lod_mesh(&t, &chunk, 1), generate_chunk_lod_mesh(&t, &chunk, 8));
        assert_eq!(max_x(&a), max_x(&b));
    }

    /// Two chunks lit their shared edge differently, each from a one-sided
    /// slope, which showed every seam as a line on a hillside.
    #[test]
    fn neighbouring_chunks_light_their_shared_edge_alike() {
        let t = TerrainData { chunks_x: 2, chunks_z: 1, ..terrain(4) };
        let res = t.chunk_resolution;
        let make = |cx: u32| {
            let mut c = TerrainChunkData::new(cx, 0, res, 0.5);
            for z in 0..res {
                for x in 0..res {
                    let gx = (cx * (res - 1) + x) as f32;
                    c.heights[(z * res + x) as usize] = 0.5 + 0.05 * (gx * 0.3).sin() + 0.002 * z as f32;
                }
            }
            c
        };
        let (a, b) = (make(0), make(1));
        let edge_normals = |mesh: &Mesh, at_x: f32| -> Vec<[f32; 3]> {
            let (Some(VertexAttributeValues::Float32x3(p)), Some(VertexAttributeValues::Float32x3(n))) =
                (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.attribute(Mesh::ATTRIBUTE_NORMAL))
            else {
                panic!("no positions or normals")
            };
            // The surface: not the floor plate's corners below it, nor the
            // outer walls' copies of the edge, which face sideways.
            // The lowest ground is 0.45 of the range; the floor sits 0.5 m under it.
            let surface = t.min_height + 0.45 * t.height_range() - 0.25;
            let mut edge: Vec<_> = p
                .iter()
                .zip(n)
                .filter(|(p, n)| p[0] == at_x && p[1] > surface && n[1] > 0.1)
                .collect();
            edge.sort_by(|l, r| l.0[2].total_cmp(&r.0[2]));
            edge.into_iter().map(|(_, n)| *n).collect()
        };
        let extent = (res - 1) as f32 * t.vertex_spacing();
        for step in [1, 4] {
            let mesh_a = generate_chunk_lod_mesh_beside(&t, &a, step, ChunkNeighbours { east: Some(&b), ..default() });
            let mesh_b = generate_chunk_lod_mesh_beside(&t, &b, step, ChunkNeighbours { west: Some(&a), ..default() });
            let (ea, eb) = (edge_normals(&mesh_a, extent), edge_normals(&mesh_b, 0.0));
            assert_eq!(ea.len(), res as usize, "step {step}");
            for (na, nb) in ea.iter().zip(&eb) {
                for i in 0..3 {
                    assert!((na[i] - nb[i]).abs() < 1e-5, "step {step}: {na:?} vs {nb:?}");
                }
            }
        }
    }

    /// The seam guarantee, on rough ground where the old skirts let pinholes
    /// through: every level carries the full-detail border vertex for vertex,
    /// at the identical float, and its surface still faces up everywhere.
    #[test]
    fn every_level_shares_the_full_detail_border_exactly() {
        let t = TerrainData { chunks_x: 3, chunks_z: 3, ..terrain(4) };
        let mut chunk = TerrainChunkData::new(1, 1, t.chunk_resolution, 0.5);
        for (i, h) in chunk.heights.iter_mut().enumerate() {
            *h = 0.5 + 0.04 * (i as f32 * 1.7).sin() + 0.02 * (i as f32 * 0.31).cos();
        }
        let positions = |m: &Mesh| match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => Vec::new(),
        };
        let extent = (t.chunk_resolution - 1) as f32 * t.vertex_spacing();
        let border = |p: &[f32; 3]| p[0] == 0.0 || p[2] == 0.0 || p[0] == extent || p[2] == extent;
        let key = |p: &[f32; 3]| p.map(f32::to_bits);
        let full = generate_chunk_lod_mesh(&t, &chunk, 1);
        let full_border: std::collections::HashSet<_> =
            positions(&full).iter().filter(|p| border(p)).map(key).collect();
        // 128 surface samples per side, plus the floor plate's 4 corners.
        assert_eq!(full_border.len(), 4 * 128 + 4);
        for step in [2, 4, 8, 64] {
            let mesh = generate_chunk_lod_mesh(&t, &chunk, step);
            let verts = positions(&mesh);
            let level: std::collections::HashSet<_> = verts.iter().filter(|p| border(p)).map(key).collect();
            assert_eq!(level, full_border, "step {step}");
            let Some(Indices::U32(indices)) = mesh.indices() else { panic!("no indices") };
            for tri in indices.chunks(3) {
                // An inner chunk has no walls, so every triangle is surface
                // or the floor plate, and both face up.
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from(verts[i as usize]));
                assert!((b - a).cross(c - a).y > 0.0, "step {step}: a triangle faces down");
            }
        }
    }
}
