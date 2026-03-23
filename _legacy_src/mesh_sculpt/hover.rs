//! Mesh sculpt hover detection via MeshRayCast

use bevy::prelude::*;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::window::PrimaryWindow;

use crate::core::{ViewportCamera, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::component_system::MeshNodeData;
use crate::terrain::TerrainSculptState;

use super::data::MeshSculptState;

/// System to detect hover position on non-terrain meshes during TerrainSculpt mode.
/// Runs after terrain hover — if terrain already has a hit, this system is skipped
/// (checked via `TerrainSculptState`).
pub fn mesh_sculpt_hover_system(
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mesh_node_query: Query<Entity, With<MeshNodeData>>,
    terrain_sculpt_state: Res<TerrainSculptState>,
    mut sculpt_state: ResMut<MeshSculptState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mesh_ray_cast: MeshRayCast,
    meshes: Res<Assets<Mesh>>,
    mesh_handles: Query<&Mesh3d>,
) {
    // Only active in terrain sculpt mode
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        sculpt_state.hover_position = None;
        sculpt_state.hover_normal = None;
        sculpt_state.active_mesh = None;
        sculpt_state.brush_visible = false;
        return;
    }

    // If terrain has a hover hit, terrain takes priority
    if terrain_sculpt_state.hover_position.is_some() {
        sculpt_state.hover_position = None;
        sculpt_state.hover_normal = None;
        sculpt_state.active_mesh = None;
        sculpt_state.brush_visible = false;
        if mouse_button.just_released(MouseButton::Left) {
            sculpt_state.is_sculpting = false;
            sculpt_state.flatten_start_distance = None;
        }
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        sculpt_state.hover_position = None;
        sculpt_state.hover_normal = None;
        return;
    };

    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        sculpt_state.hover_position = None;
        sculpt_state.hover_normal = None;
        return;
    }

    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        sculpt_state.hover_position = None;
        return;
    };

    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        sculpt_state.hover_position = None;
        return;
    };

    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings::default());

    // Find closest hit on a MeshNodeData entity
    let mut best: Option<(Entity, Vec3, Vec3, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        if !mesh_node_query.contains(*hit_entity) {
            continue;
        }

        // Interpolate normal from vertex normals via barycentric coords
        let normal = interpolate_hit_normal(
            *hit_entity,
            hit.triangle_index,
            &hit.barycentric_coords,
            &mesh_handles,
            &meshes,
        )
        .unwrap_or(Vec3::Y);

        if best.is_none() || hit.distance < best.as_ref().unwrap().3 {
            best = Some((*hit_entity, hit.point, normal, hit.distance));
        }
    }

    if let Some((entity, point, normal, _)) = best {
        sculpt_state.hover_position = Some(point);
        sculpt_state.hover_normal = Some(normal);
        sculpt_state.active_mesh = Some(entity);
        sculpt_state.brush_visible = true;

        if mouse_button.just_pressed(MouseButton::Left) {
            sculpt_state.is_sculpting = true;
        }
    } else {
        sculpt_state.hover_position = None;
        sculpt_state.hover_normal = None;
        sculpt_state.active_mesh = None;
        sculpt_state.brush_visible = false;
    }

    if mouse_button.just_released(MouseButton::Left) {
        sculpt_state.is_sculpting = false;
        sculpt_state.flatten_start_distance = None;
    }
}

/// Interpolate vertex normal at a ray hit using barycentric coordinates.
fn interpolate_hit_normal(
    entity: Entity,
    triangle_index: Option<usize>,
    barycentric: &Vec3,
    mesh_handles: &Query<&Mesh3d>,
    meshes: &Assets<Mesh>,
) -> Option<Vec3> {
    let tri_idx = triangle_index?;
    let mesh_handle = mesh_handles.get(entity).ok()?;
    let mesh = meshes.get(&mesh_handle.0)?;

    let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)?;
    let norm_values: Vec<[f32; 3]> = match normals {
        bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => return None,
    };

    let indices = mesh.indices()?;
    let idx_base = tri_idx * 3;
    let (i0, i1, i2) = match indices {
        bevy::mesh::Indices::U16(v) => {
            if idx_base + 2 >= v.len() {
                return None;
            }
            (
                v[idx_base] as usize,
                v[idx_base + 1] as usize,
                v[idx_base + 2] as usize,
            )
        }
        bevy::mesh::Indices::U32(v) => {
            if idx_base + 2 >= v.len() {
                return None;
            }
            (
                v[idx_base] as usize,
                v[idx_base + 1] as usize,
                v[idx_base + 2] as usize,
            )
        }
    };

    if i0 >= norm_values.len() || i1 >= norm_values.len() || i2 >= norm_values.len() {
        return None;
    }

    let n0 = Vec3::from(norm_values[i0]);
    let n1 = Vec3::from(norm_values[i1]);
    let n2 = Vec3::from(norm_values[i2]);

    let interpolated = n0 * barycentric.x + n1 * barycentric.y + n2 * barycentric.z;
    Some(interpolated.normalize_or_zero())
}
