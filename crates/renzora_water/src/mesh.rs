use bevy::prelude::*;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

/// Generate a flat subdivided XZ plane centered at origin.
/// All wave displacement happens in the vertex shader.
pub fn generate_water_mesh(size: f32, subdivisions: u32) -> Mesh {
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
