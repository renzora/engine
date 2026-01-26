//! Physics collision shape gizmos

use bevy::prelude::*;
use bevy::math::Isometry3d;

use crate::core::{CollisionGizmoVisibility, EditorSettings, SceneManagerState, SelectionState, ViewportCamera, ViewportState};
use crate::shared::{CollisionShapeData, CollisionShapeType, PhysicsBodyData};
use super::picking::{get_cursor_ray, ray_plane_intersection};
use super::{ColliderEditHandle, GizmoState, SelectionGizmoGroup};

const HANDLE_PICK_RADIUS: f32 = 0.2;

/// Draw collision shape gizmos based on visibility settings
pub fn draw_physics_gizmos(
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    settings: Res<EditorSettings>,
    selection: Res<SelectionState>,
    collision_shapes: Query<(Entity, &GlobalTransform, &CollisionShapeData)>,
    physics_bodies: Query<(Entity, &GlobalTransform), With<PhysicsBodyData>>,
) {
    let shape_color = Color::srgba(0.4, 0.9, 0.5, 0.8);
    let body_color = Color::srgba(0.3, 0.7, 0.9, 0.6);

    let show_all = settings.collision_gizmo_visibility == CollisionGizmoVisibility::Always;

    // Draw collision shapes based on visibility setting
    for (entity, transform, shape) in collision_shapes.iter() {
        if !show_all && !selection.is_selected(entity) {
            continue;
        }
        let entity_pos = transform.translation();
        let rotation = transform.to_scale_rotation_translation().1;
        // Apply offset in local space
        let pos = entity_pos + rotation * shape.offset;

        match shape.shape_type {
            CollisionShapeType::Box => {
                draw_box_gizmo(&mut gizmos, pos, rotation, shape.half_extents, shape_color);
            }
            CollisionShapeType::Sphere => {
                draw_sphere_gizmo(&mut gizmos, pos, shape.radius, shape_color);
            }
            CollisionShapeType::Capsule => {
                draw_capsule_gizmo(&mut gizmos, pos, rotation, shape.radius, shape.half_height, shape_color);
            }
            CollisionShapeType::Cylinder => {
                draw_cylinder_gizmo(&mut gizmos, pos, rotation, shape.radius, shape.half_height, shape_color);
            }
        }
    }

    // Draw a small indicator for physics bodies without shapes
    for (entity, transform) in physics_bodies.iter() {
        if !show_all && !selection.is_selected(entity) {
            continue;
        }
        let pos = transform.translation();
        // Draw a small cross to indicate physics body position
        let size = 0.2;
        gizmos.line(pos - Vec3::X * size, pos + Vec3::X * size, body_color);
        gizmos.line(pos - Vec3::Y * size, pos + Vec3::Y * size, body_color);
        gizmos.line(pos - Vec3::Z * size, pos + Vec3::Z * size, body_color);
    }
}

/// Exit collider edit mode when the edited entity is deselected
pub fn collider_edit_selection_sync(
    mut gizmo_state: ResMut<GizmoState>,
    selection: Res<SelectionState>,
) {
    if let Some(entity) = gizmo_state.collider_edit.entity {
        if !selection.is_selected(entity) {
            gizmo_state.collider_edit.stop_editing();
        }
    }
}

/// Draw collider edit handles when in edit mode
pub fn draw_collider_edit_handles(
    gizmo_state: Res<GizmoState>,
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    collision_shapes: Query<(&GlobalTransform, &CollisionShapeData)>,
) {
    let Some(entity) = gizmo_state.collider_edit.entity else {
        return;
    };

    let Ok((transform, shape)) = collision_shapes.get(entity) else {
        return;
    };

    let entity_pos = transform.translation();
    let rotation = transform.to_scale_rotation_translation().1;
    let center = entity_pos + rotation * shape.offset;

    // Axis colors (matching transform gizmo)
    let x_color = Color::srgb(0.9, 0.2, 0.2);
    let y_color = Color::srgb(0.2, 0.9, 0.2);
    let z_color = Color::srgb(0.2, 0.4, 0.9);
    let hovered_color = Color::srgb(1.0, 1.0, 0.5);
    let center_color = Color::srgb(0.2, 0.8, 1.0);
    let handle_size = 0.12;

    let hovered = gizmo_state.collider_edit.hovered_handle;

    // Helper to get axis color
    let get_handle_color = |handle: ColliderEditHandle| -> Color {
        if hovered == Some(handle) {
            return hovered_color;
        }
        match handle {
            ColliderEditHandle::Center => center_color,
            ColliderEditHandle::PosX | ColliderEditHandle::NegX => x_color,
            ColliderEditHandle::PosY | ColliderEditHandle::NegY => y_color,
            ColliderEditHandle::PosZ | ColliderEditHandle::NegZ => z_color,
        }
    };

    // Draw center handle (for moving offset) - filled cube appearance
    let center_color = get_handle_color(ColliderEditHandle::Center);
    draw_filled_cube(&mut gizmos, center, handle_size * 1.5, center_color);

    // Draw resize handles based on shape type
    match shape.shape_type {
        CollisionShapeType::Box => {
            // 6 face handles for box
            let handles = [
                (ColliderEditHandle::PosX, rotation * Vec3::X * shape.half_extents.x),
                (ColliderEditHandle::NegX, rotation * Vec3::NEG_X * shape.half_extents.x),
                (ColliderEditHandle::PosY, rotation * Vec3::Y * shape.half_extents.y),
                (ColliderEditHandle::NegY, rotation * Vec3::NEG_Y * shape.half_extents.y),
                (ColliderEditHandle::PosZ, rotation * Vec3::Z * shape.half_extents.z),
                (ColliderEditHandle::NegZ, rotation * Vec3::NEG_Z * shape.half_extents.z),
            ];

            for (handle_type, offset) in handles {
                let handle_pos = center + offset;
                draw_filled_cube(&mut gizmos, handle_pos, handle_size, get_handle_color(handle_type));
            }
        }
        CollisionShapeType::Sphere => {
            // 6 handles at radius distance
            let handles = [
                (ColliderEditHandle::PosX, Vec3::X * shape.radius),
                (ColliderEditHandle::NegX, Vec3::NEG_X * shape.radius),
                (ColliderEditHandle::PosY, Vec3::Y * shape.radius),
                (ColliderEditHandle::NegY, Vec3::NEG_Y * shape.radius),
                (ColliderEditHandle::PosZ, Vec3::Z * shape.radius),
                (ColliderEditHandle::NegZ, Vec3::NEG_Z * shape.radius),
            ];

            for (handle_type, offset) in handles {
                let handle_pos = center + offset;
                draw_filled_sphere(&mut gizmos, handle_pos, handle_size * 0.5, get_handle_color(handle_type));
            }
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            let up = rotation * Vec3::Y;

            // Top and bottom handles for height
            let top_handle = center + up * shape.half_height;
            let bottom_handle = center - up * shape.half_height;

            draw_filled_cube(&mut gizmos, top_handle, handle_size, get_handle_color(ColliderEditHandle::PosY));
            draw_filled_cube(&mut gizmos, bottom_handle, handle_size, get_handle_color(ColliderEditHandle::NegY));

            // Side handles for radius (X and Z directions at center height)
            let side_handles = [
                (ColliderEditHandle::PosX, rotation * Vec3::X * shape.radius),
                (ColliderEditHandle::NegX, rotation * Vec3::NEG_X * shape.radius),
                (ColliderEditHandle::PosZ, rotation * Vec3::Z * shape.radius),
                (ColliderEditHandle::NegZ, rotation * Vec3::NEG_Z * shape.radius),
            ];

            for (handle_type, offset) in side_handles {
                let handle_pos = center + offset;
                draw_filled_sphere(&mut gizmos, handle_pos, handle_size * 0.5, get_handle_color(handle_type));
            }
        }
    }
}

/// System to detect hovering over collider edit handles
pub fn collider_edit_hover_system(
    mut gizmo_state: ResMut<GizmoState>,
    viewport: Res<ViewportState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    collision_shapes: Query<(&GlobalTransform, &CollisionShapeData)>,
) {
    // Only run when in collider edit mode
    if !gizmo_state.collider_edit.is_active() {
        return;
    }

    // Don't update hover while dragging
    if gizmo_state.collider_edit.is_dragging {
        return;
    }

    gizmo_state.collider_edit.hovered_handle = None;

    if !viewport.hovered {
        return;
    }

    let Some(entity) = gizmo_state.collider_edit.entity else {
        return;
    };

    let Ok((transform, shape)) = collision_shapes.get(entity) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let entity_pos = transform.translation();
    let rotation = transform.to_scale_rotation_translation().1;
    let center = entity_pos + rotation * shape.offset;

    // Build list of handles and their positions
    let mut handles: Vec<(ColliderEditHandle, Vec3)> = vec![
        (ColliderEditHandle::Center, center),
    ];

    match shape.shape_type {
        CollisionShapeType::Box => {
            handles.extend([
                (ColliderEditHandle::PosX, center + rotation * Vec3::X * shape.half_extents.x),
                (ColliderEditHandle::NegX, center + rotation * Vec3::NEG_X * shape.half_extents.x),
                (ColliderEditHandle::PosY, center + rotation * Vec3::Y * shape.half_extents.y),
                (ColliderEditHandle::NegY, center + rotation * Vec3::NEG_Y * shape.half_extents.y),
                (ColliderEditHandle::PosZ, center + rotation * Vec3::Z * shape.half_extents.z),
                (ColliderEditHandle::NegZ, center + rotation * Vec3::NEG_Z * shape.half_extents.z),
            ]);
        }
        CollisionShapeType::Sphere => {
            handles.extend([
                (ColliderEditHandle::PosX, center + Vec3::X * shape.radius),
                (ColliderEditHandle::NegX, center + Vec3::NEG_X * shape.radius),
                (ColliderEditHandle::PosY, center + Vec3::Y * shape.radius),
                (ColliderEditHandle::NegY, center + Vec3::NEG_Y * shape.radius),
                (ColliderEditHandle::PosZ, center + Vec3::Z * shape.radius),
                (ColliderEditHandle::NegZ, center + Vec3::NEG_Z * shape.radius),
            ]);
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            let up = rotation * Vec3::Y;
            handles.extend([
                (ColliderEditHandle::PosY, center + up * shape.half_height),
                (ColliderEditHandle::NegY, center - up * shape.half_height),
                (ColliderEditHandle::PosX, center + rotation * Vec3::X * shape.radius),
                (ColliderEditHandle::NegX, center + rotation * Vec3::NEG_X * shape.radius),
                (ColliderEditHandle::PosZ, center + rotation * Vec3::Z * shape.radius),
                (ColliderEditHandle::NegZ, center + rotation * Vec3::NEG_Z * shape.radius),
            ]);
        }
    }

    // Find closest handle to ray
    let mut closest_handle: Option<ColliderEditHandle> = None;
    let mut closest_dist = f32::MAX;

    for (handle_type, handle_pos) in handles {
        // Simple sphere intersection test
        let to_handle = handle_pos - ray.origin;
        let projected = to_handle.dot(*ray.direction);
        if projected < 0.0 {
            continue; // Handle is behind camera
        }
        let closest_point = ray.origin + *ray.direction * projected;
        let dist = (closest_point - handle_pos).length();

        if dist < HANDLE_PICK_RADIUS && dist < closest_dist {
            closest_dist = dist;
            closest_handle = Some(handle_type);
        }
    }

    gizmo_state.collider_edit.hovered_handle = closest_handle;
}

/// System to handle clicking and dragging collider edit handles
pub fn collider_edit_interaction_system(
    mut gizmo_state: ResMut<GizmoState>,
    viewport: Res<ViewportState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    _windows: Query<&Window>,
    _camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    collision_shapes: Query<(&GlobalTransform, &CollisionShapeData)>,
) {
    // Only run when in collider edit mode
    if !gizmo_state.collider_edit.is_active() {
        return;
    }

    // Handle mouse release - end drag
    if mouse_button.just_released(MouseButton::Left) {
        gizmo_state.collider_edit.is_dragging = false;
        gizmo_state.collider_edit.drag_handle = None;
        return;
    }

    if !viewport.hovered {
        return;
    }

    // Handle click to start drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some(handle) = gizmo_state.collider_edit.hovered_handle {
            if let Some(entity) = gizmo_state.collider_edit.entity {
                if let Ok((_, shape)) = collision_shapes.get(entity) {
                    gizmo_state.collider_edit.is_dragging = true;
                    gizmo_state.collider_edit.drag_handle = Some(handle);
                    gizmo_state.collider_edit.drag_start_offset = shape.offset;
                    gizmo_state.collider_edit.drag_start_size = Vec3::new(
                        shape.half_extents.x,
                        shape.half_height,
                        shape.radius,
                    );
                }
            }
        } else {
            // Clicked outside handles - exit edit mode
            gizmo_state.collider_edit.stop_editing();
        }
    }
}

/// Find closest point on an axis line to a ray, with camera fallback for stability
fn ray_to_axis_closest_point_stable(ray: &Ray3d, axis_origin: Vec3, axis_dir: Vec3, cam_forward: Vec3) -> Vec3 {
    let w0 = ray.origin - axis_origin;
    let a = ray.direction.dot(*ray.direction);
    let b = ray.direction.dot(axis_dir);
    let c = axis_dir.dot(axis_dir);
    let d = ray.direction.dot(w0);
    let e = axis_dir.dot(w0);

    let denom = a * c - b * b;

    // If ray is nearly parallel to axis (denom too small), use plane intersection instead
    // This prevents jittering when looking along the axis
    if denom.abs() < 0.01 {
        // Use a plane perpendicular to camera but containing the axis
        let plane_normal = cam_forward.cross(axis_dir).cross(axis_dir).normalize_or_zero();
        if plane_normal.length_squared() < 0.001 {
            // Fallback: use camera forward as plane normal
            if let Some(hit) = ray_plane_intersection(ray, axis_origin, cam_forward) {
                // Project hit point onto axis
                let to_hit = hit - axis_origin;
                let t = to_hit.dot(axis_dir) / axis_dir.length_squared();
                return axis_origin + axis_dir * t;
            }
            return axis_origin;
        }
        if let Some(hit) = ray_plane_intersection(ray, axis_origin, plane_normal) {
            // Project hit point onto axis
            let to_hit = hit - axis_origin;
            let t = to_hit.dot(axis_dir) / axis_dir.length_squared();
            return axis_origin + axis_dir * t;
        }
        return axis_origin;
    }

    let t_axis = (a * e - b * d) / denom;
    axis_origin + axis_dir * t_axis
}

/// System to handle dragging collider edit handles
pub fn collider_edit_drag_system(
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mut collision_shapes: Query<(&GlobalTransform, &mut CollisionShapeData)>,
    mut scene_state: ResMut<SceneManagerState>,
) {
    if !gizmo_state.collider_edit.is_dragging || !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let Some(entity) = gizmo_state.collider_edit.entity else {
        return;
    };

    let Some(handle) = gizmo_state.collider_edit.drag_handle else {
        return;
    };

    let Ok((transform, mut shape)) = collision_shapes.get_mut(entity) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let Ok((_, cam_transform)) = camera_query.single() else {
        return;
    };

    let entity_pos = transform.translation();
    let rotation = transform.to_scale_rotation_translation().1;
    let center = entity_pos + rotation * shape.offset;

    // Mark scene as modified
    scene_state.mark_modified();

    match handle {
        ColliderEditHandle::Center => {
            // Drag to move offset - use camera-facing plane
            let cam_forward = *cam_transform.forward();
            if let Some(hit) = ray_plane_intersection(&ray, center, cam_forward) {
                // Convert world delta to local offset
                let world_delta = hit - center;
                let local_delta = rotation.inverse() * world_delta;
                shape.offset = gizmo_state.collider_edit.drag_start_offset + local_delta;
            }
        }
        ColliderEditHandle::PosX | ColliderEditHandle::NegX => {
            // Drag along X axis using stable axis-constrained picking
            let axis = rotation * Vec3::X;
            let cam_forward = *cam_transform.forward();
            let closest_point = ray_to_axis_closest_point_stable(&ray, center, axis, cam_forward);
            let dist = (closest_point - center).dot(axis);
            let new_size = dist.abs().max(0.05);

            match shape.shape_type {
                CollisionShapeType::Box => {
                    shape.half_extents.x = new_size;
                }
                CollisionShapeType::Sphere => {
                    shape.radius = new_size;
                }
                CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
                    shape.radius = new_size;
                }
            }
        }
        ColliderEditHandle::PosY | ColliderEditHandle::NegY => {
            // Drag along Y axis using stable axis-constrained picking
            let axis = rotation * Vec3::Y;
            let cam_forward = *cam_transform.forward();
            let closest_point = ray_to_axis_closest_point_stable(&ray, center, axis, cam_forward);
            let dist = (closest_point - center).dot(axis);
            let new_size = dist.abs().max(0.05);

            match shape.shape_type {
                CollisionShapeType::Box => {
                    shape.half_extents.y = new_size;
                }
                CollisionShapeType::Sphere => {
                    shape.radius = new_size;
                }
                CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
                    shape.half_height = new_size;
                }
            }
        }
        ColliderEditHandle::PosZ | ColliderEditHandle::NegZ => {
            // Drag along Z axis using stable axis-constrained picking
            let axis = rotation * Vec3::Z;
            let cam_forward = *cam_transform.forward();
            let closest_point = ray_to_axis_closest_point_stable(&ray, center, axis, cam_forward);
            let dist = (closest_point - center).dot(axis);
            let new_size = dist.abs().max(0.05);

            match shape.shape_type {
                CollisionShapeType::Box => {
                    shape.half_extents.z = new_size;
                }
                CollisionShapeType::Sphere => {
                    shape.radius = new_size;
                }
                CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
                    shape.radius = new_size;
                }
            }
        }
    }
}

/// Draw a filled cube handle (using multiple overlapping lines to simulate fill)
fn draw_filled_cube(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, size: f32, color: Color) {
    let half = size * 0.5;
    let steps = 3; // Number of fill lines per face

    // Draw the cube wireframe
    gizmos.cuboid(Transform::from_translation(pos).with_scale(Vec3::splat(size)), color);

    // Fill each face with diagonal lines
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let offset = half * (2.0 * t - 1.0);

        // XY faces (front and back)
        gizmos.line(pos + Vec3::new(-half, offset, half), pos + Vec3::new(half, offset, half), color);
        gizmos.line(pos + Vec3::new(-half, offset, -half), pos + Vec3::new(half, offset, -half), color);

        // XZ faces (top and bottom)
        gizmos.line(pos + Vec3::new(-half, half, offset), pos + Vec3::new(half, half, offset), color);
        gizmos.line(pos + Vec3::new(-half, -half, offset), pos + Vec3::new(half, -half, offset), color);

        // YZ faces (left and right)
        gizmos.line(pos + Vec3::new(half, -half, offset), pos + Vec3::new(half, half, offset), color);
        gizmos.line(pos + Vec3::new(-half, -half, offset), pos + Vec3::new(-half, half, offset), color);
    }
}

/// Draw a filled sphere handle (using multiple circles to simulate fill)
fn draw_filled_sphere(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, radius: f32, color: Color) {
    // Draw multiple circles at different orientations to create a filled appearance
    let circle_count = 4;
    for i in 0..circle_count {
        let angle = (i as f32 / circle_count as f32) * std::f32::consts::PI;
        let rotation = Quat::from_rotation_y(angle);
        let iso = Isometry3d::new(pos, rotation);
        gizmos.circle(iso, radius, color);
    }

    // Add horizontal circles at different heights
    for i in 0..=2 {
        let t = i as f32 / 2.0;
        let y_offset = radius * (2.0 * t - 1.0) * 0.7;
        let r = (radius * radius - y_offset * y_offset).sqrt().max(0.01);
        let iso = Isometry3d::new(pos + Vec3::Y * y_offset, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
        gizmos.circle(iso, r, color);
    }
}

/// Draw a wireframe box
fn draw_box_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, half_extents: Vec3, color: Color) {
    let corners = [
        Vec3::new(-half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x, -half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x, -half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x,  half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x,  half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x,  half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x,  half_extents.y,  half_extents.z),
    ];

    let transformed: Vec<Vec3> = corners.iter().map(|c| pos + rotation * *c).collect();

    // Bottom face
    gizmos.line(transformed[0], transformed[1], color);
    gizmos.line(transformed[1], transformed[2], color);
    gizmos.line(transformed[2], transformed[3], color);
    gizmos.line(transformed[3], transformed[0], color);

    // Top face
    gizmos.line(transformed[4], transformed[5], color);
    gizmos.line(transformed[5], transformed[6], color);
    gizmos.line(transformed[6], transformed[7], color);
    gizmos.line(transformed[7], transformed[4], color);

    // Vertical edges
    gizmos.line(transformed[0], transformed[4], color);
    gizmos.line(transformed[1], transformed[5], color);
    gizmos.line(transformed[2], transformed[6], color);
    gizmos.line(transformed[3], transformed[7], color);
}

/// Draw a wireframe sphere (3 circles)
fn draw_sphere_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, radius: f32, color: Color) {
    // XY plane circle
    let xy_iso = Isometry3d::new(pos, Quat::IDENTITY);
    gizmos.circle(xy_iso, radius, color);

    // XZ plane circle
    let xz_iso = Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(xz_iso, radius, color);

    // YZ plane circle
    let yz_iso = Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    gizmos.circle(yz_iso, radius, color);
}

/// Draw a wireframe capsule
fn draw_capsule_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    // Draw top and bottom circles
    let top_iso = Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    let bottom_iso = Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(top_iso, radius, color);
    gizmos.circle(bottom_iso, radius, color);

    // Draw vertical lines connecting the circles
    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);

    // Draw hemisphere arcs on top and bottom
    draw_hemisphere_arcs(gizmos, top_center, rotation, radius, true, color);
    draw_hemisphere_arcs(gizmos, bottom_center, rotation, radius, false, color);
}

/// Draw a wireframe cylinder
fn draw_cylinder_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    // Draw top and bottom circles
    let top_iso = Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    let bottom_iso = Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(top_iso, radius, color);
    gizmos.circle(bottom_iso, radius, color);

    // Draw vertical lines connecting the circles
    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);
}

/// Helper to draw hemisphere arcs for capsule ends
fn draw_hemisphere_arcs(gizmos: &mut Gizmos<SelectionGizmoGroup>, center: Vec3, rotation: Quat, radius: f32, top: bool, color: Color) {
    let segments = 8;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;
    let up = rotation * Vec3::Y;

    let sign = if top { 1.0 } else { -1.0 };

    // Draw arc in XY plane (from side view)
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;

        let p1 = center + right * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p2 = center + right * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p1, p2, color);

        let p3 = center - right * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p4 = center - right * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p3, p4, color);
    }

    // Draw arc in ZY plane (from front view)
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;

        let p1 = center + forward * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p2 = center + forward * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p1, p2, color);

        let p3 = center - forward * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p4 = center - forward * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p3, p4, color);
    }
}
