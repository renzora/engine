//! Foliage runtime systems — mesh rebuilding and uniform updates.

use bevy::prelude::*;

use renzora_terrain::data::TerrainData;

use crate::data::{FoliageBatch, FoliageConfig, FoliageDensityMap};
use crate::material::GrassMaterial;
use crate::mesh_gen::generate_foliage_chunk_mesh;

/// Rebuilds foliage meshes when a chunk's density map is marked dirty.
pub fn foliage_mesh_rebuild_system(
    mut commands: Commands,
    foliage_config: Res<FoliageConfig>,
    mut density_query: Query<(
        Entity,
        &mut FoliageDensityMap,
        &renzora_terrain::data::TerrainChunkData,
        &GlobalTransform,
    )>,
    terrain_query: Query<&TerrainData>,
    existing_batches: Query<(Entity, &FoliageBatch)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    for (chunk_entity, mut density_map, chunk_data, chunk_transform) in density_query.iter_mut() {
        if !density_map.dirty {
            continue;
        }
        density_map.dirty = false;

        // Find parent terrain data
        let terrain = terrain_query.iter().next();
        let Some(terrain_data) = terrain else {
            continue;
        };

        // Remove existing foliage batches for this chunk
        for (batch_entity, batch) in existing_batches.iter() {
            if batch.chunk_entity == chunk_entity {
                commands.entity(batch_entity).despawn();
            }
        }

        let chunk_world = chunk_transform.translation();

        // Generate mesh for each foliage type
        for (type_idx, foliage_type) in foliage_config.types.iter().enumerate() {
            let mesh = generate_foliage_chunk_mesh(
                foliage_type,
                type_idx,
                &density_map,
                &chunk_data.heights,
                terrain_data.chunk_resolution,
                terrain_data.chunk_size,
                terrain_data.min_height,
                terrain_data.max_height - terrain_data.min_height,
                chunk_data.chunk_x * 1000 + chunk_data.chunk_z,
            );

            let Some(mesh) = mesh else {
                continue;
            };

            let mesh_handle = meshes.add(mesh);
            let mut mat = GrassMaterial::default();
            mat.uniforms.color_base = Vec4::new(
                foliage_type.color_base.red,
                foliage_type.color_base.green,
                foliage_type.color_base.blue,
                1.0,
            );
            mat.uniforms.color_tip = Vec4::new(
                foliage_type.color_tip.red,
                foliage_type.color_tip.green,
                foliage_type.color_tip.blue,
                1.0,
            );
            mat.uniforms.wind_strength = foliage_type.wind_strength;
            mat.uniforms.chunk_world_x = chunk_world.x;
            mat.uniforms.chunk_world_z = chunk_world.z;
            let mat_handle = materials.add(mat);

            commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(mat_handle),
                Transform::from_translation(chunk_world),
                Visibility::default(),
                FoliageBatch {
                    foliage_type_index: type_idx,
                    chunk_entity,
                },
            ));
        }
    }
}

/// Updates time and wind uniforms on all grass materials each frame.
pub fn foliage_uniform_update_system(
    time: Res<Time>,
    batch_query: Query<&MeshMaterial3d<GrassMaterial>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    let t = time.elapsed_secs();
    for mat_handle in batch_query.iter() {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.uniforms.time = t;
        }
    }
}

/// When terrain chunks are sculpted (heightmap changes), mark their foliage
/// density maps as dirty so the grass mesh rebuilds at the new heights.
pub fn foliage_follow_terrain_system(
    mut query: Query<
        (&renzora_terrain::data::TerrainChunkData, &mut FoliageDensityMap),
        Changed<renzora_terrain::data::TerrainChunkData>,
    >,
) {
    for (chunk, mut density_map) in query.iter_mut() {
        if chunk.dirty {
            density_map.dirty = true;
        }
    }
}
