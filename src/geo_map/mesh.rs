use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

/// Generate a flat quad mesh for a single map tile
pub fn generate_tile_quad(tile_world_size: f32) -> Mesh {
    let half = tile_world_size / 2.0;

    let positions: Vec<[f32; 3]> = vec![
        [-half, 0.0, -half],
        [ half, 0.0, -half],
        [ half, 0.0,  half],
        [-half, 0.0,  half],
    ];

    let normals: Vec<[f32; 3]> = vec![
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];

    let uvs: Vec<[f32; 2]> = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];

    let indices = vec![0u32, 2, 1, 0, 3, 2];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
