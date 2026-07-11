//! Chunk residency streaming — drop far chunks' mesh + collider by camera
//! distance, rebuild them on approach.
//!
//! A chunk's heavy state is its render mesh (`Assets<Mesh>` entry, ~33k tris
//! at default resolution) and its avian trimesh collider; the heightmap
//! (`TerrainChunkData`) is comparatively small and *is* the authored data, so
//! it must never be dropped. Streaming out therefore removes `Mesh3d` (the
//! only strong handle — the asset frees) and tears down the physics
//! components, marking the chunk [`TerrainChunkStreamedOut`]. Streaming in is
//! just removing that marker: `rehydrate_terrain_chunks` already rebuilds any
//! chunk `Without<Mesh3d>` from its heights, and re-inserting the physics
//! data components re-triggers the collider build. The marker's whole job is
//! to hold rehydrate off while the chunk is meant to be absent.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::data::{TerrainChunkOf, TerrainChunkStreamedOut, TerrainData};

/// Exclusive driver for chunk residency. Cheap when no terrain streams:
/// one config pass over terrains, one distance check per chunk.
pub fn stream_terrain_chunks(world: &mut World) {
    // Outside streaming (editor edit mode, dedicated server) every chunk must
    // be resident — restore any that were streamed out when play mode ended.
    if !renzora::world_streaming_active(world) {
        let stale: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<TerrainChunkStreamedOut>>();
            q.iter(world).collect()
        };
        for entity in stale {
            world.entity_mut(entity).remove::<TerrainChunkStreamedOut>();
        }
        return;
    }
    let Some(camera_pos) = renzora::streaming_camera_pos(world) else {
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
                // Streaming switched off mid-play — bring everything back.
                if streamed_out {
                    to_stream_in.push(entity);
                }
                continue;
            }
            // Chunk transforms sit at the chunk's corner (see
            // `chunk_world_origin`); measure from the center.
            let center =
                transform.transform_point(Vec3::new(chunk_size * 0.5, 0.0, chunk_size * 0.5));
            let dist = camera_pos.distance(center);
            if streamed_out {
                if dist <= radius {
                    to_stream_in.push(entity);
                }
            } else if has_mesh && dist > radius + chunk_size {
                // Stream-out one chunk_size beyond the resident radius —
                // hysteresis so a boundary chunk doesn't thrash.
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
            commands
                .entity(entity)
                .remove::<Mesh3d>()
                .try_insert(TerrainChunkStreamedOut);
        }
        world.flush();
    }
}
