use bevy::prelude::*;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};

use crate::meshes::{GizmoMesh, GizmoRoot};
use crate::modal_transform::ModalTransformState;
use crate::picking::{
    get_cursor_ray, ray_box_intersection, ray_circle_intersection_point, ray_plane_intersection,
    ray_quad_intersection, ray_to_axis_closest_point, ray_to_axis_distance, ray_to_circle_distance,
};
use crate::state::{DragAxis, EditorTool, GizmoMode, GizmoState, SnapSettings, SnapTarget};
use crate::{GIZMO_CENTER_SIZE, GIZMO_PICK_THRESHOLD, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE};
use renzora_editor::{EditorSelection, HideInHierarchy};
use renzora_runtime::EditorCamera;
use renzora_viewport::ViewportState;

pub fn gizmo_hover_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<EditorSelection>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    transforms: Query<&Transform>,
) {
    if modal.active {
        gizmo.hovered_axis = None;
        return;
    }

    if gizmo.is_dragging {
        return;
    }

    gizmo.hovered_axis = None;

    if gizmo.tool != EditorTool::Transform {
        return;
    }

    if !viewport.hovered {
        return;
    }

    let Some(selected) = selection.get() else {
        return;
    };

    let Ok(obj_transform) = transforms.get(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let pos = obj_transform.translation;
    let gs = gizmo.gizmo_scale;

    match gizmo.mode {
        GizmoMode::Translate => {
            // Check plane handles first
            let plane_center_xy = pos + Vec3::new(GIZMO_PLANE_OFFSET * gs, GIZMO_PLANE_OFFSET * gs, 0.0);
            let plane_center_xz = pos + Vec3::new(GIZMO_PLANE_OFFSET * gs, 0.0, GIZMO_PLANE_OFFSET * gs);
            let plane_center_yz = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET * gs, GIZMO_PLANE_OFFSET * gs);

            let xy_hit = ray_quad_intersection(&ray, plane_center_xy, Vec3::Z, GIZMO_PLANE_SIZE * gs * 0.5);
            let xz_hit = ray_quad_intersection(&ray, plane_center_xz, Vec3::Y, GIZMO_PLANE_SIZE * gs * 0.5);
            let yz_hit = ray_quad_intersection(&ray, plane_center_yz, Vec3::X, GIZMO_PLANE_SIZE * gs * 0.5);

            let mut best_plane: Option<(DragAxis, f32)> = None;
            if let Some(t) = xy_hit {
                best_plane = Some((DragAxis::XY, t));
            }
            if let Some(t) = xz_hit {
                if best_plane.is_none() || t < best_plane.unwrap().1 {
                    best_plane = Some((DragAxis::XZ, t));
                }
            }
            if let Some(t) = yz_hit {
                if best_plane.is_none() || t < best_plane.unwrap().1 {
                    best_plane = Some((DragAxis::YZ, t));
                }
            }

            if let Some((axis, _)) = best_plane {
                gizmo.hovered_axis = Some(axis);
                return;
            }

            // Check center cube
            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE * gs).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
                return;
            }

            // Check axes
            let effective_size = GIZMO_SIZE * gs;
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * effective_size);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * effective_size);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * effective_size);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD * gs {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }
        }
        GizmoMode::Rotate => {
            let radius = GIZMO_SIZE * gs * 0.7;
            let threshold = GIZMO_PICK_THRESHOLD * gs * 1.5;

            let x_dist = ray_to_circle_distance(&ray, pos, Vec3::X, radius);
            let y_dist = ray_to_circle_distance(&ray, pos, Vec3::Y, radius);
            let z_dist = ray_to_circle_distance(&ray, pos, Vec3::Z, radius);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < threshold {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }
        }
        GizmoMode::Scale => {
            let effective_size = GIZMO_SIZE * gs;
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * effective_size);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * effective_size);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * effective_size);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD * gs {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }

            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE * gs).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
            }
        }
    }
}

pub fn gizmo_interaction_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<EditorSelection>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    transforms: Query<&Transform>,
) {
    if modal.active {
        return;
    }

    // Handle mouse release
    if mouse_button.just_released(MouseButton::Left) {
        if gizmo.is_dragging {
            gizmo.is_dragging = false;
            gizmo.drag_axis = None;
            gizmo.drag_start_transform = None;
            gizmo.drag_entity = None;
            gizmo.snap_target = SnapTarget::None;
            gizmo.snap_target_position = None;
        }
        return;
    }

    if !viewport.hovered {
        return;
    }

    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // If hovering over a gizmo axis, start drag
    if gizmo.hovered_axis.is_some() {
        if let Some(axis) = gizmo.hovered_axis {
            if let Some(selected) = selection.get() {
                if let Ok(obj_transform) = transforms.get(selected) {
                    if let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) {
                        let pos = obj_transform.translation;
                        let cam_transform = camera_query.single().map(|(_, t)| t);

                        match gizmo.mode {
                            GizmoMode::Translate => {
                                let drag_point = match axis {
                                    DragAxis::X => ray_to_axis_closest_point(&ray, pos, Vec3::X),
                                    DragAxis::Y => ray_to_axis_closest_point(&ray, pos, Vec3::Y),
                                    DragAxis::Z => ray_to_axis_closest_point(&ray, pos, Vec3::Z),
                                    DragAxis::XY => ray_plane_intersection(&ray, pos, Vec3::Z).unwrap_or(pos),
                                    DragAxis::XZ => ray_plane_intersection(&ray, pos, Vec3::Y).unwrap_or(pos),
                                    DragAxis::YZ => ray_plane_intersection(&ray, pos, Vec3::X).unwrap_or(pos),
                                    DragAxis::Free => {
                                        if let Ok(cam_t) = cam_transform {
                                            let cam_forward = cam_t.forward();
                                            ray_plane_intersection(&ray, pos, *cam_forward).unwrap_or(pos)
                                        } else {
                                            ray_plane_intersection(&ray, pos, *ray.direction).unwrap_or(pos)
                                        }
                                    }
                                };
                                gizmo.drag_start_offset = drag_point - pos;
                            }
                            GizmoMode::Rotate => {
                                gizmo.drag_start_rotation = obj_transform.rotation;
                                let axis_vec = match axis {
                                    DragAxis::X => Vec3::X,
                                    DragAxis::Y => Vec3::Y,
                                    DragAxis::Z => Vec3::Z,
                                    _ => Vec3::Y,
                                };
                                if let Some(hit_point) = ray_circle_intersection_point(&ray, pos, axis_vec) {
                                    let to_hit = (hit_point - pos).normalize();
                                    gizmo.drag_start_angle = to_hit.x.atan2(to_hit.z);
                                    gizmo.drag_start_offset = to_hit;
                                }
                            }
                            GizmoMode::Scale => {
                                gizmo.drag_start_scale = obj_transform.scale;
                                let axis_vec = match axis {
                                    DragAxis::X => Vec3::X,
                                    DragAxis::Y => Vec3::Y,
                                    DragAxis::Z => Vec3::Z,
                                    DragAxis::Free | _ => Vec3::ONE,
                                };
                                let drag_point = ray_to_axis_closest_point(&ray, pos, axis_vec);
                                gizmo.drag_start_distance = (drag_point - pos).length();
                                gizmo.drag_start_offset = drag_point - pos;
                            }
                        }

                        gizmo.drag_start_transform = Some(*obj_transform);
                        gizmo.drag_entity = Some(selected);
                    }
                }
            }

            gizmo.is_dragging = true;
            gizmo.drag_axis = Some(axis);
        }
    }
}

pub fn object_drag_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<EditorSelection>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut transforms: Query<&mut Transform>,
    all_transforms: Query<(Entity, &GlobalTransform)>,
) {
    if modal.active {
        return;
    }

    if !gizmo.is_dragging || !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let Some(selected) = selection.get() else {
        return;
    };

    let Ok(mut obj_transform) = transforms.get_mut(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else { return };

    let cam_global = camera_query.single().ok().map(|(_, t)| t);

    let ctrl_held = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    let snap = if ctrl_held {
        SnapSettings {
            translate_enabled: true,
            translate_snap: gizmo.snap.translate_snap.max(1.0),
            rotate_enabled: true,
            rotate_snap: gizmo.snap.rotate_snap.max(15.0),
            scale_enabled: true,
            scale_snap: gizmo.snap.scale_snap.max(0.25),
            object_snap_enabled: gizmo.snap.object_snap_enabled,
            object_snap_distance: gizmo.snap.object_snap_distance,
            floor_snap_enabled: gizmo.snap.floor_snap_enabled,
            floor_y: gizmo.snap.floor_y,
        }
    } else {
        gizmo.snap
    };

    match gizmo.mode {
        GizmoMode::Translate => {
            let current_pos = obj_transform.translation;
            let offset = gizmo.drag_start_offset;

            let new_pos = match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::X);
                    Vec3::new(point.x - offset.x, current_pos.y, current_pos.z)
                }
                Some(DragAxis::Y) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::Y);
                    Vec3::new(current_pos.x, point.y - offset.y, current_pos.z)
                }
                Some(DragAxis::Z) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::Z);
                    Vec3::new(current_pos.x, current_pos.y, point.z - offset.z)
                }
                Some(DragAxis::XY) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::Z) {
                        Vec3::new(point.x - offset.x, point.y - offset.y, current_pos.z)
                    } else { current_pos }
                }
                Some(DragAxis::XZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::Y) {
                        Vec3::new(point.x - offset.x, current_pos.y, point.z - offset.z)
                    } else { current_pos }
                }
                Some(DragAxis::YZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::X) {
                        Vec3::new(current_pos.x, point.y - offset.y, point.z - offset.z)
                    } else { current_pos }
                }
                Some(DragAxis::Free) => {
                    let plane_normal = cam_global.map(|t| *t.forward()).unwrap_or(*ray.direction);
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, plane_normal) {
                        point - offset
                    } else { current_pos }
                }
                None => current_pos,
            };

            let mut final_pos = snap.snap_translate_vec3(new_pos);

            // Clear snap target
            gizmo.snap_target = SnapTarget::None;
            gizmo.snap_target_position = None;

            // Object snapping
            if gizmo.snap.object_snap_enabled {
                let snap_distance = gizmo.snap.object_snap_distance;
                let mut closest_entity: Option<Entity> = None;
                let mut closest_distance = f32::MAX;
                let mut closest_pos = Vec3::ZERO;

                for (other_entity, other_global) in all_transforms.iter() {
                    if Some(other_entity) == gizmo.drag_entity {
                        continue;
                    }
                    let other_pos = other_global.translation();
                    let distance = (final_pos - other_pos).length();
                    if distance < snap_distance && distance < closest_distance {
                        closest_distance = distance;
                        closest_entity = Some(other_entity);
                        closest_pos = other_pos;
                    }
                }

                if let Some(entity) = closest_entity {
                    match gizmo.drag_axis {
                        Some(DragAxis::X) => {
                            if (final_pos.x - closest_pos.x).abs() < snap_distance {
                                final_pos.x = closest_pos.x;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Y) => {
                            if (final_pos.y - closest_pos.y).abs() < snap_distance {
                                final_pos.y = closest_pos.y;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Z) => {
                            if (final_pos.z - closest_pos.z).abs() < snap_distance {
                                final_pos.z = closest_pos.z;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Free) | None => {
                            final_pos = closest_pos;
                            gizmo.snap_target = SnapTarget::Entity(entity);
                            gizmo.snap_target_position = Some(closest_pos);
                        }
                        _ => {
                            // Multi-axis snap
                            let mut snapped = false;
                            if matches!(gizmo.drag_axis, Some(DragAxis::XY) | Some(DragAxis::XZ)) {
                                if (final_pos.x - closest_pos.x).abs() < snap_distance {
                                    final_pos.x = closest_pos.x;
                                    snapped = true;
                                }
                            }
                            if matches!(gizmo.drag_axis, Some(DragAxis::XY) | Some(DragAxis::YZ)) {
                                if (final_pos.y - closest_pos.y).abs() < snap_distance {
                                    final_pos.y = closest_pos.y;
                                    snapped = true;
                                }
                            }
                            if matches!(gizmo.drag_axis, Some(DragAxis::XZ) | Some(DragAxis::YZ)) {
                                if (final_pos.z - closest_pos.z).abs() < snap_distance {
                                    final_pos.z = closest_pos.z;
                                    snapped = true;
                                }
                            }
                            if snapped {
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                    }
                }
            }

            // Floor snapping
            if gizmo.snap.floor_snap_enabled && gizmo.snap_target == SnapTarget::None {
                let floor_y = gizmo.snap.floor_y;
                let snap_distance = gizmo.snap.object_snap_distance;
                let affects_y = matches!(
                    gizmo.drag_axis,
                    Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ) | Some(DragAxis::Free) | None
                );
                if affects_y && (final_pos.y - floor_y).abs() < snap_distance {
                    final_pos.y = floor_y;
                    gizmo.snap_target = SnapTarget::Floor;
                    gizmo.snap_target_position = Some(Vec3::new(final_pos.x, floor_y, final_pos.z));
                }
            }

            obj_transform.translation = final_pos;
        }
        GizmoMode::Rotate => {
            let pos = obj_transform.translation;
            let axis_vec = match gizmo.drag_axis {
                Some(DragAxis::X) => Vec3::X,
                Some(DragAxis::Y) => Vec3::Y,
                Some(DragAxis::Z) => Vec3::Z,
                _ => return,
            };

            if let Some(hit_point) = ray_circle_intersection_point(&ray, pos, axis_vec) {
                let to_hit = (hit_point - pos).normalize();
                let start_dir = gizmo.drag_start_offset;

                let cross = start_dir.cross(to_hit);
                let dot = start_dir.dot(to_hit);
                let angle = cross.dot(axis_vec).atan2(dot);

                let snapped_angle = snap.snap_rotate(angle);
                let rotation_delta = Quat::from_axis_angle(axis_vec, snapped_angle);
                obj_transform.rotation = rotation_delta * gizmo.drag_start_rotation;
            }
        }
        GizmoMode::Scale => {
            let pos = obj_transform.translation;
            let start_scale = gizmo.drag_start_scale;
            let start_dist = gizmo.drag_start_distance.max(0.001);

            match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::X);
                    let current_dist = (point - pos).dot(Vec3::X).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.x * scale_factor).max(0.01);
                    obj_transform.scale.x = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Y) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Y);
                    let current_dist = (point - pos).dot(Vec3::Y).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.y * scale_factor).max(0.01);
                    obj_transform.scale.y = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Z) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Z);
                    let current_dist = (point - pos).dot(Vec3::Z).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.z * scale_factor).max(0.01);
                    obj_transform.scale.z = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Free) => {
                    let cam_forward = cam_global.map(|t| *t.forward()).unwrap_or(*ray.direction);
                    if let Some(point) = ray_plane_intersection(&ray, pos, cam_forward) {
                        let current_dist = (point - pos).length();
                        let scale_factor = current_dist / start_dist;
                        let new_scale = (start_scale * scale_factor).max(Vec3::splat(0.01));
                        obj_transform.scale = snap.snap_scale_vec3(new_scale);
                    }
                }
                _ => {}
            }
        }
    }
}

/// System that picks entities by clicking in the viewport using mesh raycasting.
pub fn entity_pick_system(
    gizmo: Res<GizmoState>,
    selection: Res<EditorSelection>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    named_entities: Query<Entity, With<Name>>,
    parent_query: Query<&ChildOf>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
) {
    if modal.active || gizmo.is_dragging {
        return;
    }

    if !viewport.hovered || !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // Don't pick entities if we clicked on a gizmo axis
    if gizmo.hovered_axis.is_some() {
        return;
    }

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings::default());

    let mut closest_entity: Option<Entity> = None;
    let mut closest_distance = f32::MAX;

    for (hit_entity, hit) in hits.iter() {
        // Skip gizmo meshes
        if gizmo_meshes.get(*hit_entity).is_ok() {
            continue;
        }
        // Skip hidden entities
        if hidden_entities.get(*hit_entity).is_ok() {
            continue;
        }

        // Find the named ancestor (the "selectable" entity)
        if let Some(named) = find_named_ancestor(*hit_entity, &named_entities, &parent_query) {
            if hit.distance < closest_distance {
                closest_distance = hit.distance;
                closest_entity = Some(named);
            }
        }
    }

    if let Some(entity) = closest_entity {
        selection.set(Some(entity));
    } else {
        // Clicked empty space — clear selection
        selection.set(None);
    }
}

/// Walk up the parent chain to find the nearest entity with a `Name` component.
fn find_named_ancestor(
    entity: Entity,
    named: &Query<Entity, With<Name>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    if named.get(entity).is_ok() {
        return Some(entity);
    }
    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        let parent = child_of.parent();
        if named.get(parent).is_ok() {
            return Some(parent);
        }
        current = parent;
    }
    None
}
