//! Terrain mesh generation

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

use super::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Generate a terrain mesh for a chunk
pub fn generate_chunk_mesh(
    terrain: &TerrainData,
    chunk: &TerrainChunkData,
) -> Mesh {
    let resolution = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let height_range = terrain.max_height - terrain.min_height;

    let vertex_count = (resolution * resolution) as usize;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    // Generate vertices
    for z in 0..resolution {
        for x in 0..resolution {
            let height_normalized = chunk.get_height(x, z, resolution);
            let height = terrain.min_height + height_normalized * height_range;

            let pos = Vec3::new(
                x as f32 * spacing,
                height,
                z as f32 * spacing,
            );
            positions.push(pos);

            // UV coordinates (0-1 across chunk)
            let u = x as f32 / (resolution - 1) as f32;
            let v = z as f32 / (resolution - 1) as f32;
            uvs.push([u, v]);

            // Placeholder normal (will be calculated after)
            normals.push(Vec3::Y);
        }
    }

    // Calculate normals from neighboring heights
    for z in 0..resolution {
        for x in 0..resolution {
            let idx = (z * resolution + x) as usize;

            // Get heights of neighboring vertices (with edge clamping)
            let h_left = if x > 0 {
                chunk.get_height(x - 1, z, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_right = if x < resolution - 1 {
                chunk.get_height(x + 1, z, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_down = if z > 0 {
                chunk.get_height(x, z - 1, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_up = if z < resolution - 1 {
                chunk.get_height(x, z + 1, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };

            // Convert to world heights for proper normal calculation
            let h_left_world = terrain.min_height + h_left * height_range;
            let h_right_world = terrain.min_height + h_right * height_range;
            let h_down_world = terrain.min_height + h_down * height_range;
            let h_up_world = terrain.min_height + h_up * height_range;

            // Calculate normal using central differences
            let dx = (h_right_world - h_left_world) / (2.0 * spacing);
            let dz = (h_up_world - h_down_world) / (2.0 * spacing);

            let normal = Vec3::new(-dx, 1.0, -dz).normalize();
            normals[idx] = normal;
        }
    }

    // Generate indices (two triangles per quad)
    let quad_count = ((resolution - 1) * (resolution - 1)) as usize;
    let mut indices = Vec::with_capacity(quad_count * 6);

    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let top_left = z * resolution + x;
            let top_right = top_left + 1;
            let bottom_left = top_left + resolution;
            let bottom_right = bottom_left + 1;

            // First triangle (top-left, bottom-left, top-right)
            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            // Second triangle (top-right, bottom-left, bottom-right)
            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
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

/// System to update terrain chunk meshes when dirty
pub fn terrain_chunk_mesh_update_system(
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_query: Query<&TerrainData>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &Mesh3d)>,
) {
    for (mut chunk, chunk_of, mesh_handle) in chunk_query.iter_mut() {
        if !chunk.dirty {
            continue;
        }

        let Ok(terrain) = terrain_query.get(chunk_of.0) else {
            continue;
        };

        // Generate new mesh
        let new_mesh = generate_chunk_mesh(terrain, &chunk);

        // Update the mesh asset
        if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }

        chunk.dirty = false;
    }
}
