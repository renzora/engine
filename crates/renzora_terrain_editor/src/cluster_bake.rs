//! Builds the clusters a terrain chunk is drawn from as virtual geometry
//! (see `renzora_terrain::clusters`).
//!
//! Here because the builder (meshoptimizer, the `bake` feature of
//! `renzora_rvg`) is in the editor and not in a game. Once a chunk's ground
//! has stopped changing for [`SETTLE_SECS`], its mesh is built into clusters
//! on a background thread and written to `<folder>/chunk_<x>_<z>.rvg`, which is
//! what a game loads. The finished file is also handed to the chunk straight
//! away (`ChunkClusters`), so the editor switches to it without reading it back
//! from disk.
//!
//! A chunk is rebuilt only when its file was built from different ground: a
//! scene opened with up-to-date files builds nothing.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use renzora::core::CurrentProject;
use renzora_rvg::bake::{build_mesh, MeshInput};
use renzora_terrain::clusters::{mesh_hash, ChunkClusters, ClusterFile, RvgFile, TerrainClusters};
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData, TerrainSculptState};
use renzora_terrain::landscape::TerrainMaterial;

/// How long a chunk's ground must sit still before its clusters are built:
/// long enough that a brush stroke builds once at the end, not every frame.
const SETTLE_SECS: f32 = 0.75;

/// Chunks built at once. Each is about 33 thousand triangles and a few tens
/// of milliseconds of one core; four keeps a whole new terrain to a few
/// seconds without starving the rest of the editor.
const MAX_BUILDS: usize = 4;

#[derive(Resource, Default)]
pub(crate) struct ClusterBakes {
    /// When each chunk's mesh last changed, in seconds since startup.
    changed_at: HashMap<Entity, f32>,
    building: HashMap<Entity, Building>,
    /// The mesh each chunk's clusters were last built from, so a build that
    /// has landed is not started again while the runtime catches up.
    built: HashMap<Entity, u64>,
}

struct Building {
    path: String,
    task: Task<Result<RvgFile, String>>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ClusterBakes>().add_systems(
        Update,
        (name_cluster_folders, build_chunk_clusters).chain().run_if(renzora::core::not_in_play_mode),
    );
}

fn virtual_geometry_on(project: Option<&CurrentProject>) -> bool {
    project.is_some_and(|p| p.config.rendering.virtual_geometry)
}

/// Give every terrain a cluster folder of its own, named once and saved with
/// the scene, so two terrains (or two scenes) never share files.
#[allow(clippy::type_complexity)]
fn name_cluster_folders(
    mut commands: Commands,
    project: Option<Res<CurrentProject>>,
    // Only a terrain with a landscape material is ever drawn as virtual
    // geometry (its blockout grid is not a material the renderer can shade).
    terrains: Query<Entity, (With<TerrainData>, With<TerrainMaterial>, Without<TerrainClusters>)>,
) {
    if !virtual_geometry_on(project.as_deref()) {
        return;
    }
    for terrain in &terrains {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        terrain.to_bits().hash(&mut h);
        std::time::SystemTime::now().hash(&mut h);
        commands
            .entity(terrain)
            .try_insert(TerrainClusters { dir: format!("terrain_clusters/{:016x}", h.finish()) });
    }
}

#[allow(clippy::too_many_arguments)]
fn build_chunk_clusters(
    time: Res<Time>,
    project: Option<Res<CurrentProject>>,
    sculpt: Option<Res<TerrainSculptState>>,
    meshes: Res<Assets<Mesh>>,
    mut files: ResMut<Assets<RvgFile>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    mut chunks: Query<(Entity, &Mesh3d, &TerrainChunkOf, Option<&mut ChunkClusters>), With<TerrainChunkData>>,
    surfaced: Query<(), With<TerrainMaterial>>,
    mut bakes: ResMut<ClusterBakes>,
) {
    let Some(project) = project.filter(|p| virtual_geometry_on(Some(p))) else {
        mesh_events.clear();
        return;
    };
    let now = time.elapsed_secs();

    let modified: Vec<AssetId<Mesh>> = mesh_events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if !modified.is_empty() {
        for (entity, mesh, _, _) in &chunks {
            if modified.contains(&mesh.id()) {
                bakes.changed_at.insert(entity, now);
            }
        }
    }

    // Builds that finished: hand each to its chunk.
    let mut finished = Vec::new();
    bakes.building.retain(|entity, b| match block_on(future::poll_once(&mut b.task)) {
        Some(result) => {
            finished.push((*entity, b.path.clone(), result));
            false
        }
        None => true,
    });
    for (entity, path, result) in finished {
        match result {
            Ok(file) => {
                bakes.built.insert(entity, file.source_hash);
                let handle = files.add(file);
                if let Ok((_, _, _, Some(mut clusters))) = chunks.get_mut(entity) {
                    if clusters.path == path {
                        clusters.state = ClusterFile::Loaded(handle);
                    }
                }
            }
            Err(err) => warn!("[terrain] could not build the virtual geometry of {path}: {err}"),
        }
    }

    // Mid-stroke, every touched chunk changes each frame; build after.
    if sculpt.is_some_and(|s| s.is_sculpting) {
        return;
    }

    let pool = AsyncComputeTaskPool::get();
    for (entity, mesh, of, clusters) in &chunks {
        if bakes.building.len() >= MAX_BUILDS {
            break;
        }
        if !surfaced.contains(of.0) {
            continue;
        }
        // The runtime sets this up (`renzora_runtime::terrain_vg`); until it
        // has, and while it is still asking whether a file exists, wait.
        let Some(clusters) = clusters else { continue };
        if matches!(clusters.state, ClusterFile::Probing(_)) || bakes.building.contains_key(&entity) {
            continue;
        }
        if bakes.changed_at.get(&entity).is_some_and(|t| now - t < SETTLE_SECS) {
            continue;
        }
        let Some(hash) = clusters.mesh_hash else { continue };
        let current = match &clusters.state {
            ClusterFile::Loaded(handle) => files.get(handle).is_some_and(|f| f.source_hash == hash),
            _ => false,
        };
        if current || bakes.built.get(&entity) == Some(&hash) {
            continue;
        }
        let Some(input) = meshes.get(&mesh.0).and_then(mesh_input) else { continue };
        // Built from what is drawn now: the mesh may have moved on since the
        // runtime last hashed it, and the file must name what it was built from.
        let Some(hash) = meshes.get(&mesh.0).and_then(mesh_hash) else { continue };
        let target: PathBuf = project.resolve_path(&clusters.path);
        let task = pool.spawn(async move { build_and_write(input, hash, target) });
        bakes.building.insert(entity, Building { path: clusters.path.clone(), task });
    }
}

/// The vertex data the builder takes, from a chunk's mesh.
fn mesh_input(mesh: &Mesh) -> Option<MeshInput> {
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => return None,
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL)? {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => return None,
    };
    let uv0 = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(v)) => v.clone(),
        _ => Vec::new(),
    };
    let indices = match mesh.indices()? {
        Indices::U32(i) => i.clone(),
        Indices::U16(i) => i.iter().map(|&i| i as u32).collect(),
    };
    // A chunk's inner edges are open and meet the next chunk's, built
    // separately: they stay at full detail so the two always match.
    Some(MeshInput { positions, normals, uv0, indices, lock_open_borders: true, ..default() })
}

fn build_and_write(input: MeshInput, hash: u64, target: PathBuf) -> Result<RvgFile, String> {
    let mesh = build_mesh(&input)?;
    let file = RvgFile {
        builder_version: renzora_rvg::BUILDER_VERSION,
        source_hash: hash,
        meshes: vec![Arc::new(mesh)],
        scene: None,
    };
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&target, file.to_bytes()).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_terrain::mesh::generate_chunk_mesh;

    #[test]
    fn a_chunk_builds_into_clusters_that_cover_its_mesh() {
        let t = TerrainData { chunk_resolution: 33, ..default() };
        let mut chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        for (i, h) in chunk.heights.iter_mut().enumerate() {
            *h = 0.5 + 0.05 * (i as f32 * 0.37).sin();
        }
        let mesh = generate_chunk_mesh(&t, &chunk);
        let input = mesh_input(&mesh).expect("a chunk mesh has every attribute the builder needs");
        let triangles = input.indices.len() / 3;
        let dir = std::env::temp_dir().join(format!("renzora_terrain_clusters_{}", std::process::id()));
        let target = dir.join("chunk_0_0.rvg");
        let file = build_and_write(input, 42, target.clone()).expect("builds");
        assert_eq!(file.source_hash, 42);
        assert_eq!(file.meshes.len(), 1);
        assert_eq!(file.meshes[0].original_triangle_count(), triangles);
        let read = RvgFile::from_bytes(&std::fs::read(&target).unwrap()).expect("reads back");
        assert_eq!(read.source_hash, 42);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Neighbouring chunks are built separately, so each keeps its edges at
    /// full detail at every level or seams open at a distance.
    #[test]
    fn the_coarsest_cut_keeps_every_edge_vertex() {
        let t = TerrainData { chunk_resolution: 65, chunks_x: 3, chunks_z: 3, ..default() };
        let mut chunk = TerrainChunkData::new(1, 1, t.chunk_resolution, 0.5);
        for (i, h) in chunk.heights.iter_mut().enumerate() {
            *h = 0.5 + 0.04 * (i as f32 * 0.37).sin() + 0.02 * (i as f32 * 0.051).cos();
        }
        let input = mesh_input(&generate_chunk_mesh(&t, &chunk)).unwrap();
        let mesh = build_mesh(&input).unwrap();
        // The coarsest cut: just under the roots' parent error, which is the
        // largest finite one (a cluster is drawn while parent error > t).
        let coarsest = mesh
            .clusters
            .iter()
            .map(|c| c.lod.error)
            .fold(0.0f32, f32::max);
        let used: std::collections::HashSet<u32> = mesh.cut_indices(coarsest).into_iter().collect();
        assert!(!used.is_empty(), "the coarsest cut draws something");
        assert!(used.len() < input.positions.len() / 2, "the coarsest cut is simplified at all");
        let extent = (t.chunk_resolution - 1) as f32 * t.vertex_spacing();
        let on_edge = |p: &[f32; 3]| p[0] == 0.0 || p[2] == 0.0 || p[0] == extent || p[2] == extent;
        let floor = input.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let edge: Vec<usize> = input
            .positions
            .iter()
            .enumerate()
            .filter(|(_, p)| on_edge(p) && p[1] > floor)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(edge.len(), 4 * 64);
        let kept = edge.iter().filter(|&&i| used.contains(&(i as u32))).count();
        assert_eq!(kept, edge.len(), "every edge vertex survives to the coarsest level");
    }
}
