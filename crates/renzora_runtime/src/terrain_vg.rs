//! Terrain chunks drawn as virtual geometry.
//!
//! Here, where both the terrain and the virtual geometry renderer are linked,
//! so neither depends on the other (as with node-graph materials, see
//! `lib.rs`). The terrain's surface material is registered with the renderer,
//! and every chunk of a terrain that has [`TerrainClusters`] loads its
//! `chunk_<x>_<z>.rvg` and is handed to the renderer for as long as that file
//! was built from the mesh the chunk has now ([`mesh_hash`]).
//!
//! Whenever it is not (a brush is changing the ground, the file is missing or
//! stale, virtual geometry is off), the chunk is drawn the ordinary way, with
//! its distance levels (`renzora_terrain::lod`). The editor builds new
//! clusters once the ground stops changing (`renzora_terrain_editor`), and the
//! chunk is handed back. Physics, shadow maps and Solari read the ordinary
//! mesh throughout, which a converted chunk keeps.

use std::collections::HashSet;

use bevy::asset::io::AssetSourceId;
use bevy::asset::AssetPath;
use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, IoTaskPool};
use renzora_terrain::clusters::{
    clusters_asset_path, mesh_hash, ChunkClusters, ClusterFile, RvgFile, TerrainClusters,
};
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_terrain::lod::{level_range, usable_levels, ChunkLods};
use renzora_terrain::surface::TerrainSurfaceMaterial;
use renzora_virtual_geometry::{
    VgMaterial, VgSurface, VirtualGeometry, VirtualGeometryAppExt, VirtualGeometrySettings,
};

pub(crate) fn install(app: &mut App) {
    app.add_virtual_geometry_material::<TerrainSurfaceMaterial>(classify_terrain_surface)
        .add_systems(PostUpdate, follow_terrain_clusters);
}

/// The terrain surface is opaque and single-sided, always.
fn classify_terrain_surface(_: &TerrainSurfaceMaterial) -> Option<VgSurface> {
    Some(VgSurface { masked: false, double_sided: false, transparent: false })
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow_terrain_clusters(
    mut commands: Commands,
    settings: Res<VirtualGeometrySettings>,
    asset_server: Res<AssetServer>,
    files: Res<Assets<RvgFile>>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    terrains: Query<(&TerrainData, Option<&TerrainClusters>)>,
    mut chunks: Query<(
        Entity,
        &Mesh3d,
        &TerrainChunkData,
        &TerrainChunkOf,
        Option<&mut ChunkClusters>,
        Has<VirtualGeometry>,
        Option<&ChunkLods>,
        Option<&VgMaterial>,
    )>,
    mut visibility: Query<&mut Visibility>,
) {
    let changed: HashSet<AssetId<Mesh>> = mesh_events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } | AssetEvent::Added { id } => Some(*id),
            _ => None,
        })
        .collect();

    for (entity, mesh, chunk, of, clusters, converted, lods, material) in &mut chunks {
        let Ok((terrain, folder)) = terrains.get(of.0) else { continue };
        let folder = folder.filter(|f| !f.dir.is_empty());
        let Some(folder) = folder.filter(|_| settings.enabled) else {
            if converted {
                hand_back(&mut commands, entity, terrain, lods, &mut visibility);
            }
            continue;
        };

        let path = folder.chunk_path(chunk.chunk_x, chunk.chunk_z);
        let Some(mut clusters) = clusters.filter(|c| c.path == path) else {
            commands.entity(entity).try_insert(ChunkClusters {
                state: probe(&asset_server, &path),
                path,
                mesh_hash: meshes.get(&mesh.0).and_then(mesh_hash),
            });
            continue;
        };

        // A file appearing later is not an asset event, since nothing had
        // loaded it: the editor, having just written one, marks it loaded
        // itself (`renzora_terrain_editor::cluster_bake`).
        if changed.contains(&mesh.id()) || clusters.mesh_hash.is_none() {
            clusters.mesh_hash = meshes.get(&mesh.0).and_then(mesh_hash);
        }
        if let ClusterFile::Probing(task) = &mut clusters.state {
            if let Some(exists) = block_on(future::poll_once(task)) {
                clusters.state = if exists {
                    ClusterFile::Loaded(asset_server.load(clusters_asset_path(&clusters.path)))
                } else {
                    ClusterFile::Missing
                };
            }
        }

        // Only a material the renderer has classified as drawable: the
        // checkerboard a terrain without a Surface wears is not registered,
        // and a chunk handed over with it would not be drawn at all.
        let drawable = material.is_some_and(|m| m.surface.is_some());
        let matches = drawable
            && match &clusters.state {
                ClusterFile::Loaded(handle) => files
                    .get(handle)
                    .is_some_and(|file| file.meshes.len() == 1 && Some(file.source_hash) == clusters.mesh_hash),
                _ => false,
            };
        match (matches, converted) {
            (true, false) => {
                let ClusterFile::Loaded(handle) = &clusters.state else { continue };
                commands
                    .entity(entity)
                    .remove::<VisibilityRange>()
                    .try_insert(VirtualGeometry { file: handle.clone(), mesh: 0, source: mesh.id() });
                set_levels_visible(lods, &mut visibility, false);
            }
            (false, true) => hand_back(&mut commands, entity, terrain, lods, &mut visibility),
            // The level meshes are respawned when the terrain's LOD settings
            // change; keep new ones hidden under a converted chunk.
            (true, true) => set_levels_visible(lods, &mut visibility, false),
            (false, false) => {}
        }
    }
}

/// Draw the chunk the ordinary way again, with its distance levels.
fn hand_back(
    commands: &mut Commands,
    chunk: Entity,
    terrain: &TerrainData,
    lods: Option<&ChunkLods>,
    visibility: &mut Query<&mut Visibility>,
) {
    let mut e = commands.entity(chunk);
    e.remove::<VirtualGeometry>();
    if lods.is_some() {
        let levels = usable_levels(terrain);
        e.try_insert(level_range(terrain, 0, levels));
    }
    set_levels_visible(lods, visibility, true);
}

fn set_levels_visible(lods: Option<&ChunkLods>, visibility: &mut Query<&mut Visibility>, visible: bool) {
    let wanted = if visible { Visibility::Inherited } else { Visibility::Hidden };
    for (level, _) in lods.map(|l| l.levels.as_slice()).unwrap_or_default() {
        if let Ok(mut v) = visibility.get_mut(*level) {
            if *v != wanted {
                *v = wanted;
            }
        }
    }
}

fn probe(asset_server: &AssetServer, path: &str) -> ClusterFile {
    let server = asset_server.clone();
    let path = AssetPath::from(path.to_string());
    ClusterFile::Probing(IoTaskPool::get().spawn(async move {
        let Ok(source) = server.get_source(AssetSourceId::Default) else {
            return false;
        };
        source.reader().read(path.path()).await.is_ok()
    }))
}
