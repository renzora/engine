use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode, ViewportCamera, SelectionState, ViewportState};

use super::picking::{
    get_cursor_ray, ray_box_intersection, ray_circle_intersection_point, ray_plane_intersection,
    ray_quad_intersection, ray_to_axis_closest_point, ray_to_axis_distance, ray_to_circle_distance,
};
use super::{DragAxis, GizmoMode, GizmoState, GIZMO_CENTER_SIZE, GIZMO_PICK_THRESHOLD, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE};

pub fn gizmo_hover_system(
    mut gizmo: ResMut<GizmoState>,
    mut selection: ResMut<SelectionState>,
    viewport: Res<ViewportState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    transforms: Query<&Transform, With<EditorEntity>>,
) {
    // Don't update hover while dragging
    if gizmo.is_dragging {
        return;
    }

    gizmo.hovered_axis = None;

    if !viewport.hovered {
        return;
    }

    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(obj_transform) = transforms.get(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let pos = obj_transform.translation;

    match gizmo.mode {
        GizmoMode::Translate => {
            // Check hits with priority: planes > axes > center
            // Check plane handles (small squares)
            let plane_center_xy = pos + Vec3::new(GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET, 0.0);
            let plane_center_xz = pos + Vec3::new(GIZMO_PLANE_OFFSET, 0.0, GIZMO_PLANE_OFFSET);
            let plane_center_yz = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET);

            let xy_hit = ray_quad_intersection(&ray, plane_center_xy, Vec3::Z, GIZMO_PLANE_SIZE * 0.5);
            let xz_hit = ray_quad_intersection(&ray, plane_center_xz, Vec3::Y, GIZMO_PLANE_SIZE * 0.5);
            let yz_hit = ray_quad_intersection(&ray, plane_center_yz, Vec3::X, GIZMO_PLANE_SIZE * 0.5);

            // Find closest plane hit
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
            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
                return;
            }

            // Check distance to each axis
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * GIZMO_SIZE);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * GIZMO_SIZE);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * GIZMO_SIZE);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD {
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
            // Check distance to each rotation circle
            let radius = GIZMO_SIZE * 0.7;
            let threshold = GIZMO_PICK_THRESHOLD * 1.5;

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
            // Check distance to each axis (same as translate but without planes)
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * GIZMO_SIZE);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * GIZMO_SIZE);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * GIZMO_SIZE);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }

            // Check center cube for uniform scale
            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
            }
        }
    }
}

pub fn gizmo_interaction_system(
    mut gizmo: ResMut<GizmoState>,
    mut selection: ResMut<SelectionState>,
    viewport: Res<ViewportState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mesh_query: Query<(Entity, &GlobalTransform), With<EditorEntity>>,
    transforms: Query<&Transform, With<EditorEntity>>,
    parents: Query<&ChildOf, With<SceneNode>>,
) {
    // Handle drag end
    if mouse_button.just_released(MouseButton::Left) {
        gizmo.is_dragging = false;
        gizmo.drag_axis = None;
        return;
    }

    if !viewport.hovered {
        return;
    }

    // Only process clicks
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // If hovering over a gizmo axis, start axis-constrained drag
    if let Some(axis) = gizmo.hovered_axis {
        // Calculate initial drag state based on gizmo mode
        if let Some(selected) = selection.selected_entity {
            if let Ok(obj_transform) = transforms.get(selected) {
                if let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) {
                    let pos = obj_transform.translation;
                    let cam_transform = camera_query.single().map(|(_, t)| t);

                    match gizmo.mode {
                        GizmoMode::Translate => {
                            // Calculate where cursor intersects the constraint
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
                                        pos
                                    }
                                }
                            };
                            gizmo.drag_start_offset = drag_point - pos;
                        }
                        GizmoMode::Rotate => {
                            // Store initial rotation and angle
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
                            // Store initial scale and distance from center
                            gizmo.drag_start_scale = obj_transform.scale;
                            let axis_vec = match axis {
                                DragAxis::X => Vec3::X,
                                DragAxis::Y => Vec3::Y,
                                DragAxis::Z => Vec3::Z,
                                DragAxis::Free => Vec3::ONE,
                                _ => Vec3::ONE,
                            };
                            let drag_point = ray_to_axis_closest_point(&ray, pos, axis_vec);
                            gizmo.drag_start_distance = (drag_point - pos).length();
                            gizmo.drag_start_offset = drag_point - pos;
                        }
                    }
                }
            }
        }

        gizmo.is_dragging = true;
        gizmo.drag_axis = Some(axis);
        return;
    }

    // Otherwise, do object picking
    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let mut closest_entity: Option<Entity> = None;
    let mut closest_distance = f32::MAX;

    for (entity, transform) in mesh_query.iter() {
        let mesh_pos = transform.translation();
        let radius = 1.0;

        let oc = ray.origin - mesh_pos;
        let a = ray.direction.dot(*ray.direction);
        let b = 2.0 * oc.dot(*ray.direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant >= 0.0 {
            let t = (-b - discriminant.sqrt()) / (2.0 * a);
            if t > 0.0 && t < closest_distance {
                closest_distance = t;
                closest_entity = Some(entity);
            }
        }
    }

    if let Some(clicked) = closest_entity {
        // Check if clicked entity is a descendant of current selection
        // If so, keep the current selection to avoid accidentally selecting children
        let should_select = if let Some(current) = selection.selected_entity {
            !is_descendant_of(clicked, current, &parents)
        } else {
            true
        };

        if should_select {
            selection.selected_entity = Some(clicked);
        }
    } else {
        selection.selected_entity = None;
    }
}

/// Check if an entity is a descendant of another entity
fn is_descendant_of(entity: Entity, ancestor: Entity, parents: &Query<&ChildOf, With<SceneNode>>) -> bool {
    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        if child_of.0 == ancestor {
            return true;
        }
        current = child_of.0;
    }
    false
}

pub fn object_drag_system(
    gizmo: Res<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mut transforms: Query<&mut Transform, With<EditorEntity>>,
) {
    if !gizmo.is_dragging || !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(mut obj_transform) = transforms.get_mut(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let Ok((_, cam_global)) = camera_query.single() else {
        return;
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
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::XZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::Y) {
                        Vec3::new(point.x - offset.x, current_pos.y, point.z - offset.z)
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::YZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::X) {
                        Vec3::new(current_pos.x, point.y - offset.y, point.z - offset.z)
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::Free) => {
                    let cam_forward = *cam_global.forward();
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, cam_forward) {
                        point - offset
                    } else {
                        current_pos
                    }
                }
                None => current_pos,
            };

            obj_transform.translation = new_pos;
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

                // Calculate angle between start direction and current direction
                let cross = start_dir.cross(to_hit);
                let dot = start_dir.dot(to_hit);
                let angle = cross.dot(axis_vec).atan2(dot);

                // Apply rotation relative to start rotation
                let rotation_delta = Quat::from_axis_angle(axis_vec, angle);
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
                    obj_transform.scale.x = (start_scale.x * scale_factor).max(0.01);
                }
                Some(DragAxis::Y) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Y);
                    let current_dist = (point - pos).dot(Vec3::Y).abs();
                    let scale_factor = current_dist / start_dist;
                    obj_transform.scale.y = (start_scale.y * scale_factor).max(0.01);
                }
                Some(DragAxis::Z) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Z);
                    let current_dist = (point - pos).dot(Vec3::Z).abs();
                    let scale_factor = current_dist / start_dist;
                    obj_transform.scale.z = (start_scale.z * scale_factor).max(0.01);
                }
                Some(DragAxis::Free) => {
                    // Uniform scale - use camera plane
                    let cam_forward = *cam_global.forward();
                    if let Some(point) = ray_plane_intersection(&ray, pos, cam_forward) {
                        let current_dist = (point - pos).length();
                        let scale_factor = current_dist / start_dist;
                        obj_transform.scale = (start_scale * scale_factor).max(Vec3::splat(0.01));
                    }
                }
                _ => {}
            }
        }
    }
}
