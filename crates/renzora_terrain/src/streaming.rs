//! Chunk residency streaming: far chunks drop their mesh and collider and are
//! drawn by a coarse stand-in until the camera comes back.
//!
//! A chunk's heavy state is its render mesh (an `Assets<Mesh>` entry, kept on
//! the CPU as well as the GPU), its trimesh collider, its levels of detail and
//! its virtual geometry; the heightmap (`TerrainChunkData`) is comparatively
//! small and *is* the authored data, so it is never dropped. Streaming out
//! therefore removes `Mesh3d` (the only strong handle, so the asset frees) and
//! the physics components, and marks the chunk [`TerrainChunkStreamedOut`].
//! Streaming in is removing that marker: `rehydrate_terrain_chunks` rebuilds
//! any chunk without a `Mesh3d` from its heights, and the physics data
//! components it re-inserts trigger the collider build.
//!
//! **A streamed-out chunk is not a hole.** It gets a [`StreamProxy`]: one small
//! mesh at the coarsest level of detail (a few hundred vertices against tens of
//! thousands), no collider, the chunk's own material. So the far side of a
//! large world is still there to look at, and to sculpt (brushes work on the
//! heights, which every chunk keeps), at a fraction of the cost.
//!
//! **It runs in the editor too, following the viewport camera.** It used to
//! run only in play mode, so editing a large world kept every chunk at full
//! detail with its collider, and the frame rate fell with the world's size
//! rather than with what was in view.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainChunkStreamedOut, TerrainData};
use crate::material::TerrainCheckerboardMaterial;
use crate::mesh::{generate_chunk_lod_mesh_beside, ChunkNeighbours};
use crate::surface::TerrainSurfaceMaterial;
use renzora::core::MaterialRef;

/// The point streaming measured from this frame, for the systems that build
/// chunks to start with the nearest (`rehydrate_terrain_chunks`,
/// `starter::shape_pending_chunks`). `None` without a camera.
#[derive(Resource, Default, Clone, Copy)]
pub struct StreamFocus(pub Option<Vec3>);

/// Exclusive driver for chunk residency. Cheap when no terrain streams:
/// one config pass over terrains, one distance check per chunk.
pub fn stream_terrain_chunks(world: &mut World) {
    // A dedicated server has no camera, and gameplay there needs the whole
    // world resident: bring back anything streamed out and stop.
    if world.get_resource::<renzora::core::DedicatedServer>().is_some() {
        let stale: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<TerrainChunkStreamedOut>>();
            q.iter(world).collect()
        };
        for entity in stale {
            world.entity_mut(entity).remove::<TerrainChunkStreamedOut>();
        }
        return;
    }
    // Editing: what the viewport is looking at. Playing, or a shipped game:
    // the gameplay camera, as every other streamer measures from.
    let camera_pos = if renzora::core::editor_editing(world) {
        renzora::core::editor_camera_pos(world)
    } else {
        renzora::streaming_camera_pos(world)
    };
    if world.get_resource::<StreamFocus>().is_none_or(|f| f.0 != camera_pos) {
        world.insert_resource(StreamFocus(camera_pos));
    }
    let Some(camera_pos) = camera_pos else {
        return;
    };

    // (streaming enabled, resident radius, chunk size) per terrain root.
    let configs: HashMap<Entity, (bool, f32, f32)> = {
        let mut q = world.query::<(Entity, &TerrainData)>();
        q.iter(world)
            .map(|(entity, terrain)| {
                (
                    entity,
                    (
                        terrain.stream_chunks,
                        // A radius under one chunk would stream out the chunk
                        // the camera stands on — clamp to something sane.
                        terrain.stream_radius.max(terrain.chunk_size),
                        terrain.chunk_size,
                    ),
                )
            })
            .collect()
    };

    let mut to_stream_in: Vec<Entity> = Vec::new();
    let mut to_stream_out: Vec<Entity> = Vec::new();
    {
        let mut q = world.query::<(
            Entity,
            &TerrainChunkOf,
            &GlobalTransform,
            Has<TerrainChunkStreamedOut>,
            Has<Mesh3d>,
        )>();
        for (entity, chunk_of, transform, streamed_out, has_mesh) in q.iter(world) {
            let Some(&(stream, radius, chunk_size)) = configs.get(&chunk_of.0) else {
                continue;
            };
            if !stream {
                // Streaming switched off: bring everything back.
                if streamed_out {
                    to_stream_in.push(entity);
                }
                continue;
            }
            // Chunk transforms sit at the chunk's corner (see
            // `chunk_world_origin`); measure from the center, across the
            // ground only, so flying high over a world does not unload it.
            let center =
                transform.transform_point(Vec3::new(chunk_size * 0.5, 0.0, chunk_size * 0.5));
            let dist = camera_pos.xz().distance(center.xz());
            if streamed_out {
                if dist <= radius {
                    to_stream_in.push(entity);
                }
            } else if (has_mesh && dist > radius + chunk_size) || (!has_mesh && dist > radius) {
                // Stream-out one chunk_size beyond the resident radius:
                // hysteresis so a boundary chunk doesn't thrash.
                //
                // A chunk with no mesh yet is one just spawned or resized,
                // which `rehydrate_terrain_chunks` would otherwise build at
                // full detail this frame. Marked out here, it never is: a
                // grid grown to 64 x 64 used to build all 4096 full meshes
                // (about 5 GB) in one frame and ran the GPU out of memory.
                to_stream_out.push(entity);
            }
        }
    }

    for entity in to_stream_in {
        world.entity_mut(entity).remove::<TerrainChunkStreamedOut>();
    }
    if !to_stream_out.is_empty() {
        let mut commands = world.commands();
        for entity in to_stream_out {
            // Collider first (both backends' component sets), then the mesh —
            // dropping `Mesh3d` releases the only strong handle to the chunk
            // mesh asset, which is the actual memory win.
            renzora_physics::despawn_physics_components(&mut commands, entity);
            // A pending collider rebuild goes too; streaming in builds one.
            commands
                .entity(entity)
                .remove::<(Mesh3d, crate::mesh::ColliderStale)>()
                .try_insert(TerrainChunkStreamedOut);
        }
        world.flush();
    }
}

/// The coarse stand-in a streamed-out chunk is drawn with.
#[derive(Component, Debug)]
pub struct StreamProxy {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}

/// The step, in full-detail vertices, a stand-in is built at: eight cells
/// across, or as close as the grid halves to. Coarser saves nothing, since a
/// stand-in keeps its border at full detail to meet its neighbours exactly
/// and the border is then most of its vertices; finer costs triangles a
/// distant chunk does not need.
fn proxy_step(terrain: &TerrainData) -> u32 {
    let cells = terrain.chunk_resolution.saturating_sub(1).max(1);
    let mut step = 1;
    while cells.is_multiple_of(step * 2) && cells / (step * 2) >= 8 {
        step *= 2;
    }
    step
}

/// Give streamed-out chunks their stand-in, take it away when they come back,
/// and keep it current while the chunk's ground is edited from afar.
#[allow(clippy::type_complexity)]
pub fn sync_stream_proxies(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrains: Query<&TerrainData>,
    mut chunks: Query<(
        Entity,
        &mut TerrainChunkData,
        &TerrainChunkOf,
        Has<TerrainChunkStreamedOut>,
        Option<&StreamProxy>,
    )>,
    materials: Query<(
        Option<&MeshMaterial3d<TerrainSurfaceMaterial>>,
        Option<&MeshMaterial3d<TerrainCheckerboardMaterial>>,
        Option<&MaterialRef>,
        Option<&MeshMaterial3d<renzora_shader::material::GraphMaterial>>,
    )>,
    // Chunks whose heights are still being generated. Their terrain builds
    // no stand-ins until all are done: one built now would be flat, or lit
    // at its edges from a neighbour that was, and keep that seam.
    pending: Query<&TerrainChunkOf, With<crate::starter::PendingShape>>,
) {
    let shaping: std::collections::HashSet<Entity> = pending.iter().map(|of| of.0).collect();
    // What needs building, decided before the heights are borrowed for it.
    let mut build: Vec<Entity> = Vec::new();
    for (entity, chunk, of, streamed_out, proxy) in &chunks {
        if shaping.contains(&of.0) {
            continue;
        }
        match (streamed_out, proxy) {
            (true, None) => build.push(entity),
            (true, Some(_)) if chunk.mesh_stale => build.push(entity),
            (false, Some(proxy)) => {
                commands.entity(proxy.entity).try_despawn();
                commands.entity(entity).remove::<StreamProxy>();
            }
            _ => {}
        }
    }
    if build.is_empty() {
        return;
    }

    let mut built: Vec<(Entity, Mesh)> = Vec::with_capacity(build.len());
    {
        let grid: HashMap<(Entity, u32, u32), &TerrainChunkData> = chunks
            .iter()
            .map(|(_, chunk, of, _, _)| ((of.0, chunk.chunk_x, chunk.chunk_z), chunk))
            .collect();
        for &entity in &build {
            let Ok((_, chunk, of, _, _)) = chunks.get(entity) else { continue };
            let Ok(terrain) = terrains.get(of.0) else { continue };
            let neighbours =
                ChunkNeighbours::around(chunk.chunk_x, chunk.chunk_z, |x, z| grid.get(&(of.0, x, z)).copied());
            built.push((entity, generate_chunk_lod_mesh_beside(terrain, chunk, proxy_step(terrain), neighbours)));
        }
    }

    for (entity, mesh) in built {
        let Ok((_, mut chunk, _, _, proxy)) = chunks.get_mut(entity) else { continue };
        // The full mesh is rebuilt from the heights when the chunk streams
        // back in, so the stand-in is all a far edit has to keep current.
        chunk.mesh_stale = false;
        if let Some(proxy) = proxy {
            if let Some(mut existing) = meshes.get_mut(&proxy.mesh) {
                *existing = mesh;
            }
            continue;
        }
        let handle = meshes.add(mesh);
        let mut child = commands.spawn((
            Mesh3d(handle.clone()),
            Transform::IDENTITY,
            Visibility::Inherited,
            // Generated from the chunk, never saved, not listed.
            renzora::HideInHierarchy,
            ChildOf(entity),
        ));
        if let Ok((surface, checker, material_ref, landscape)) = materials.get(entity) {
            if let Some(m) = surface {
                child.insert(m.clone());
            }
            if let Some(m) = checker {
                child.insert(m.clone());
            }
            if let Some(r) = material_ref {
                child.insert(r.clone());
            } else if let Some(m) = landscape {
                // The terrain's own landscape instance; its handle never
                // changes, so the copy stays right (`landscape::instance`).
                child.insert(m.clone());
            }
        }
        let proxy = child.id();
        commands.entity(entity).try_insert(StreamProxy { entity: proxy, mesh: handle });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stand_in_is_a_small_fraction_of_the_chunk() {
        let t = TerrainData::default();
        assert_eq!(proxy_step(&t), 16, "128 cells to 8");
        assert_eq!(proxy_step(&TerrainData { chunk_resolution: 100, ..default() }), 1, "99 cells do not halve");
        let chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        let count = |m: &Mesh| m.count_vertices();
        let full = generate_chunk_lod_mesh_beside(&t, &chunk, 1, ChunkNeighbours::default());
        let proxy = generate_chunk_lod_mesh_beside(&t, &chunk, proxy_step(&t), ChunkNeighbours::default());
        assert!(count(&proxy) * 10 < count(&full), "{} vs {}", count(&proxy), count(&full));
    }
}
