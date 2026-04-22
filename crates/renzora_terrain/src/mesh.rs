//! Terrain mesh generation — heightmap to triangle mesh with normals via central differences.

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use std::collections::HashMap;
use renzora::console_log::console_info;

use renzora_physics::{CollisionShapeData, PhysicsBodyData};

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::material::TerrainCheckerboardMaterial;
use renzora::MaterialRef;

/// Generate a triangle mesh for a single terrain chunk from its heightmap.
pub fn generate_chunk_mesh(terrain: &TerrainData, chunk: &TerrainChunkData) -> Mesh {
    let resolution = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let height_range = terrain.height_range();

    let vertex_count = (resolution * resolution) as usize;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = vec![Vec3::Y; vertex_count];
    let mut uvs = Vec::with_capacity(vertex_count);

    // Generate vertices
    for z in 0..resolution {
        for x in 0..resolution {
            let h = terrain.min_height + chunk.get_height(x, z, resolution) * height_range;
            positions.push(Vec3::new(x as f32 * spacing, h, z as f32 * spacing));
            uvs.push([
                x as f32 / (resolution - 1) as f32,
                z as f32 / (resolution - 1) as f32,
            ]);
        }
    }

    // Calculate normals using central differences
    for z in 0..resolution {
        for x in 0..resolution {
            let h_left = chunk.get_height(x.saturating_sub(1), z, resolution);
            let h_right = chunk.get_height((x + 1).min(resolution - 1), z, resolution);
            let h_down = chunk.get_height(x, z.saturating_sub(1), resolution);
            let h_up = chunk.get_height(x, (z + 1).min(resolution - 1), resolution);

            let hl = terrain.min_height + h_left * height_range;
            let hr = terrain.min_height + h_right * height_range;
            let hd = terrain.min_height + h_down * height_range;
            let hu = terrain.min_height + h_up * height_range;

            let dx = (hr - hl) / (2.0 * spacing);
            let dz = (hu - hd) / (2.0 * spacing);

            normals[(z * resolution + x) as usize] = Vec3::new(-dx, 1.0, -dz).normalize();
        }
    }

    // Generate indices (two triangles per quad)
    let quad_count = ((resolution - 1) * (resolution - 1)) as usize;
    let mut indices = Vec::with_capacity(quad_count * 6);

    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let tl = z * resolution + x;
            let tr = tl + 1;
            let bl = tl + resolution;
            let br = bl + 1;

            indices.push(tl);
            indices.push(bl);
            indices.push(tr);

            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        positions.iter().map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        normals.iter().map(|n| [n.x, n.y, n.z]).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}


/// Spawn a complete terrain entity with chunk children.
///
/// Returns the root terrain entity. Each chunk is spawned as a child with
/// the checkerboard material and a trimesh collider.
pub fn spawn_terrain(world: &mut World) -> Entity {
    // Start with a single tile; users grow the terrain via the Size tab's
    // Add Neighbor buttons (Unity-style). `TerrainData::default()` is kept at
    // 4×4 so saved scenes load unchanged.
    let terrain_data = TerrainData {
        chunks_x: 1,
        chunks_z: 1,
        ..TerrainData::default()
    };

    let material = {
        let mut mats = world.resource_mut::<Assets<TerrainCheckerboardMaterial>>();
        mats.add(TerrainCheckerboardMaterial::default())
    };

    // Build chunk data + meshes
    let mut chunks: Vec<(TerrainChunkData, Handle<Mesh>, Vec3)> = Vec::new();
    {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let chunk = TerrainChunkData::new(cx, cz, terrain_data.chunk_resolution, 0.2);
                let mesh = generate_chunk_mesh(&terrain_data, &chunk);
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);
                chunks.push((chunk, mesh_handle, origin));
            }
        }
    }

    // Y=-2.0 so terrain surface sits on the grid at default 20% height
    let terrain_entity = world
        .spawn((
            Name::new("Terrain"),
            Transform::from_xyz(0.0, -2.0, 0.0),
            Visibility::default(),
            terrain_data,
            renzora::SelectionStop,
        ))
        .id();

    console_info("Terrain", format!("Spawning terrain with {} chunks", chunks.len()));

    for (mut chunk_data, mesh_handle, origin) in chunks {
        chunk_data.dirty = false;
        let cx = chunk_data.chunk_x;
        let cz = chunk_data.chunk_z;
        let chunk_entity = world
            .spawn((
                Name::new(format!("Chunk ({},{})", cx, cz)),
                Mesh3d(mesh_handle),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(origin),
                PhysicsBodyData::static_body(),
                CollisionShapeData::mesh(),
                chunk_data,
                TerrainChunkOf(terrain_entity),
            ))
            .id();
        // Insert ChildOf separately to trigger Bevy's hierarchy hooks
        // (on_insert hooks don't fire when ChildOf is part of a spawn bundle)
        world.entity_mut(chunk_entity).insert(ChildOf(terrain_entity));
    }

    terrain_entity
}

/// Rehydrate terrain chunks after scene load — spawns mesh, material, collider,
/// and `TerrainChunkOf` for chunks that have `TerrainChunkData` but no `Mesh3d`.
pub fn rehydrate_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    terrain_query: Query<&TerrainData>,
    chunk_query: Query<
        (Entity, &TerrainChunkData, Option<&TerrainChunkOf>, Option<&ChildOf>, Option<&MaterialRef>),
        Without<Mesh3d>,
    >,
) {
    if chunk_query.is_empty() {
        return;
    }

    let material = materials.add(TerrainCheckerboardMaterial::default());

    for (entity, chunk_data, chunk_of, child_of, mat_ref) in chunk_query.iter() {
        // Resolve parent terrain: prefer TerrainChunkOf, fall back to ChildOf parent
        let parent = chunk_of
            .map(|c| c.0)
            .or_else(|| child_of.map(|c| c.0));
        let Some(parent_entity) = parent else {
            continue;
        };
        let Ok(terrain_data) = terrain_query.get(parent_entity) else {
            continue;
        };

        // Ensure parent has required hierarchy components (scene loader may omit them)
        commands.entity(parent_entity).insert((
            GlobalTransform::default(),
            InheritedVisibility::default(),
        ));

        let mesh = generate_chunk_mesh(terrain_data, chunk_data);
        let mesh_handle = meshes.add(mesh);

        let mut ec = commands.entity(entity);
        ec.insert((
            Mesh3d(mesh_handle),
            PhysicsBodyData::static_body(),
            CollisionShapeData::mesh(),
        ));

        // Only apply default checkerboard if chunk has no custom material assigned
        if mat_ref.is_none() {
            ec.insert(MeshMaterial3d(material.clone()));
        }

        // Restore TerrainChunkOf if missing (scene load doesn't serialize it)
        if chunk_of.is_none() {
            ec.insert(TerrainChunkOf(parent_entity));
        }
    }
}

/// System that regenerates meshes and colliders for dirty chunks.
pub fn terrain_chunk_mesh_update_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_query: Query<&TerrainData>,
    mut chunk_query: Query<(Entity, &mut TerrainChunkData, &TerrainChunkOf, &Mesh3d)>,
) {
    for (entity, mut chunk, chunk_of, mesh_handle) in chunk_query.iter_mut() {
        if !chunk.dirty {
            continue;
        }
        let Ok(terrain) = terrain_query.get(chunk_of.0) else {
            continue;
        };
        let new_mesh = generate_chunk_mesh(terrain, &chunk);
        if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }
        // Re-inserting triggers Changed<CollisionShapeData>, which makes the
        // physics layer rebuild the avian trimesh from the updated Mesh asset.
        commands.entity(entity).try_insert(CollisionShapeData::mesh());
        chunk.dirty = false;
    }
}

/// Bilinear resampling of heightmap from one resolution to another.
fn resample_heights(old_heights: &[f32], old_res: u32, new_res: u32) -> Vec<f32> {
    let new_size = (new_res * new_res) as usize;
    let mut out = vec![0.2f32; new_size];

    if old_heights.is_empty() || old_res < 2 || new_res < 2 {
        return out;
    }

    for nz in 0..new_res {
        for nx in 0..new_res {
            let fx = nx as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;
            let fz = nz as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;

            let ox0 = (fx.floor() as u32).min(old_res - 1);
            let oz0 = (fz.floor() as u32).min(old_res - 1);
            let ox1 = (ox0 + 1).min(old_res - 1);
            let oz1 = (oz0 + 1).min(old_res - 1);

            let tx = fx.fract();
            let tz = fz.fract();

            let get =
                |x: u32, z: u32| old_heights.get((z * old_res + x) as usize).copied().unwrap_or(0.2);

            let h = get(ox0, oz0) * (1.0 - tx) * (1.0 - tz)
                + get(ox1, oz0) * tx * (1.0 - tz)
                + get(ox0, oz1) * (1.0 - tx) * tz
                + get(ox1, oz1) * tx * tz;

            out[(nz * new_res + nx) as usize] = h;
        }
    }

    out
}

/// System that rebuilds terrain chunks when `TerrainData` changes in the inspector.
///
/// Handles chunk grid changes, resolution changes, height range changes.
/// Resamples heightmaps via bilinear interpolation when resolution changes.
pub fn terrain_data_changed_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    changed_terrain: Query<(Entity, &TerrainData), Changed<TerrainData>>,
    added: Query<Entity, Added<TerrainData>>,
    chunk_query: Query<(Entity, &TerrainChunkOf, &TerrainChunkData)>,
) {
    for (terrain_entity, terrain_data) in changed_terrain.iter() {
        // Skip first insertion — chunks are spawned by the terrain creation command
        if added.contains(terrain_entity) {
            continue;
        }

        // Collect existing chunks for this terrain
        let existing: Vec<(Entity, u32, u32, Vec<f32>)> = chunk_query
            .iter()
            .filter(|(_, of, _)| of.0 == terrain_entity)
            .map(|(e, _, data)| (e, data.chunk_x, data.chunk_z, data.base_heights.clone()))
            .collect();

        if existing.is_empty() {
            continue;
        }

        // Detect old resolution from existing chunk heightmap size
        let old_res = {
            let len = existing[0].3.len();
            (len as f32).sqrt().round() as u32
        };

        // Build lookup: (cx, cz) -> old heights
        let old_map: HashMap<(u32, u32), Vec<f32>> = existing
            .iter()
            .map(|(_, cx, cz, h)| ((*cx, *cz), h.clone()))
            .collect();

        // Despawn all old chunk entities
        for (chunk_entity, _, _, _) in &existing {
            commands.entity(*chunk_entity).despawn();
        }

        let material = terrain_materials.add(TerrainCheckerboardMaterial::default());

        let new_res = terrain_data.chunk_resolution;

        // Spawn new chunks with resampled or fresh heights
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let base_heights = if let Some(old_h) = old_map.get(&(cx, cz)) {
                    if old_res != new_res {
                        resample_heights(old_h, old_res, new_res)
                    } else {
                        old_h.clone()
                    }
                } else {
                    vec![0.2f32; (new_res * new_res) as usize]
                };

                let chunk_data = TerrainChunkData {
                    chunk_x: cx,
                    chunk_z: cz,
                    heights: base_heights.clone(),
                    base_heights,
                    dirty: false,
                };

                let mesh = generate_chunk_mesh(terrain_data, &chunk_data);
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);

                commands.entity(terrain_entity).with_child((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::mesh(),
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                ));
            }
        }
    }
}
