//! Mesh sculpt system — applies brush to mesh vertices

use bevy::prelude::*;

use crate::core::ViewportState;
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{
    TerrainBrushType, TerrainSettings, compute_brush_falloff,
};

use super::data::{MeshSculptData, MeshSculptState};

/// System that applies the sculpt brush to mesh vertices each frame.
pub fn mesh_sculpt_system(
    gizmo_state: Res<GizmoState>,
    mut sculpt_state: ResMut<MeshSculptState>,
    settings: Res<TerrainSettings>,
    viewport: Res<ViewportState>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    mesh_handles: Query<&Mesh3d>,
    global_transforms: Query<&GlobalTransform>,
    mut sculpt_data_query: Query<&mut MeshSculptData>,
) {
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        return;
    }

    if !sculpt_state.is_sculpting || !viewport.hovered {
        return;
    }

    let Some(hover_pos) = sculpt_state.hover_position else {
        return;
    };

    let Some(entity) = sculpt_state.active_mesh else {
        return;
    };

    // Lazy init: attach MeshSculptData if missing
    if sculpt_data_query.get(entity).is_err() {
        let Ok(mesh_handle) = mesh_handles.get(entity) else {
            return;
        };
        let Some(mesh) = meshes.get(&mesh_handle.0) else {
            return;
        };
        let Some(data) = MeshSculptData::from_mesh(mesh) else {
            return;
        };
        commands.entity(entity).insert(data);
        // Return this frame — data will be available next frame
        return;
    }

    let Ok(global_transform) = global_transforms.get(entity) else {
        return;
    };

    let Ok(mut sculpt_data) = sculpt_data_query.get_mut(entity) else {
        return;
    };

    let inverse_transform = global_transform.affine().inverse();
    let brush_radius = settings.brush_radius;
    let strength = settings.brush_strength * time.delta_secs() * 2.0;
    let shift_held =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // For flatten: capture starting displacement on first frame
    if sculpt_state.flatten_start_distance.is_none() {
        if matches!(settings.brush_type, TerrainBrushType::Flatten) {
            // Find the nearest vertex to hover point and use its displacement
            let local_hover = inverse_transform.transform_point3(hover_pos);
            let mut closest_dist = f32::MAX;
            let mut closest_disp = 0.0f32;
            for (i, pos) in sculpt_data.original_positions.iter().enumerate() {
                let d = Vec3::from(*pos).distance(local_hover);
                if d < closest_dist {
                    closest_dist = d;
                    closest_disp = sculpt_data.displacements[i];
                }
            }
            sculpt_state.flatten_start_distance = Some(closest_disp);
        }
    }

    // Transform hover position to local space
    let local_hover = inverse_transform.transform_point3(hover_pos);
    // Compute local-space brush radius accounting for scale
    let scale = global_transform.affine().transform_vector3(Vec3::X).length();
    let local_brush_radius = brush_radius / scale.max(0.001);

    for i in 0..sculpt_data.original_positions.len() {
        let orig_pos = Vec3::from(sculpt_data.original_positions[i]);
        let normal = Vec3::from(sculpt_data.original_normals[i]);
        // Current displaced position
        let current_pos = orig_pos + normal * sculpt_data.displacements[i];

        let dx = current_pos.x - local_hover.x;
        let dy = current_pos.y - local_hover.y;
        let dz = current_pos.z - local_hover.z;

        // 3D distance for brush influence
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if dist > local_brush_radius {
            continue;
        }

        let t = dist / local_brush_radius;
        let falloff =
            compute_brush_falloff(t, settings.falloff, settings.falloff_type);
        let effect = strength * falloff;

        match settings.brush_type {
            TerrainBrushType::Raise => {
                sculpt_data.displacements[i] += effect;
            }
            TerrainBrushType::Lower => {
                sculpt_data.displacements[i] -= effect;
            }
            TerrainBrushType::Sculpt => {
                if shift_held {
                    sculpt_data.displacements[i] -= effect;
                } else {
                    sculpt_data.displacements[i] += effect;
                }
            }
            TerrainBrushType::Smooth => {
                // Lerp displacement toward neighbor average
                let neighbors = &sculpt_data.adjacency[i];
                if !neighbors.is_empty() {
                    let avg: f32 = neighbors
                        .iter()
                        .map(|&ni| sculpt_data.displacements[ni])
                        .sum::<f32>()
                        / neighbors.len() as f32;
                    sculpt_data.displacements[i] +=
                        (avg - sculpt_data.displacements[i]) * effect * 2.0;
                }
            }
            TerrainBrushType::Flatten => {
                if let Some(target) = sculpt_state.flatten_start_distance {
                    let current = sculpt_data.displacements[i];
                    sculpt_data.displacements[i] +=
                        (target - current) * effect * 2.0;
                }
            }
            TerrainBrushType::Erase => {
                // Lerp toward 0 (original shape)
                let current = sculpt_data.displacements[i];
                sculpt_data.displacements[i] += (0.0 - current) * effect * 2.0;
            }
            // Other terrain-specific brush types are no-ops on meshes
            _ => {}
        }

        sculpt_data.dirty = true;
    }
}
