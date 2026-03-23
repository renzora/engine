//! GPU mesh update system for mesh sculpting

use bevy::prelude::*;

use super::data::{recompute_normals, MeshSculptData};

/// System that updates the GPU mesh asset when sculpt data is dirty.
///
/// Computes `position[i] = original[i] + normal[i] * displacement[i]`,
/// recomputes normals via face-normal averaging, and writes both attributes
/// back to the mesh asset in-place (topology unchanged).
pub fn mesh_sculpt_update_system(
    mut meshes: ResMut<Assets<Mesh>>,
    mut sculpt_query: Query<(&mut MeshSculptData, &Mesh3d)>,
) {
    for (mut data, mesh_handle) in sculpt_query.iter_mut() {
        if !data.dirty {
            continue;
        }

        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        // Compute displaced positions
        let new_positions: Vec<[f32; 3]> = data
            .original_positions
            .iter()
            .zip(data.original_normals.iter())
            .zip(data.displacements.iter())
            .map(|((pos, norm), &disp)| {
                [
                    pos[0] + norm[0] * disp,
                    pos[1] + norm[1] * disp,
                    pos[2] + norm[2] * disp,
                ]
            })
            .collect();

        // Recompute normals from the displaced positions
        let new_normals = recompute_normals(&new_positions, mesh.indices());

        // Update mesh attributes in-place
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, new_normals);

        data.dirty = false;
    }
}
