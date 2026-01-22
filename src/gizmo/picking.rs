use bevy::prelude::*;

use crate::core::{ViewportCamera, ViewportState};

/// Get ray from cursor position in viewport
pub fn get_cursor_ray(
    viewport: &ViewportState,
    windows: &Query<&Window>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
) -> Option<Ray3d> {
    let window = windows.single().ok()?;
    let cursor_pos = window.cursor_position()?;

    let viewport_pos = viewport.position;
    let viewport_size = viewport.size;

    let local_x = cursor_pos.x - viewport_pos[0];
    let local_y = cursor_pos.y - viewport_pos[1];

    if local_x < 0.0 || local_y < 0.0 || local_x > viewport_size[0] || local_y > viewport_size[1] {
        return None;
    }

    // The texture now matches the viewport size, so use local coordinates directly
    let viewport_cursor = Vec2::new(local_x, local_y);

    let (camera, camera_transform) = camera_query.single().ok()?;
    camera.viewport_to_world(camera_transform, viewport_cursor).ok()
}

/// Distance from ray to line segment (for gizmo axis picking)
pub fn ray_to_axis_distance(ray: &Ray3d, axis_start: Vec3, axis_end: Vec3) -> f32 {
    let axis_dir = (axis_end - axis_start).normalize();
    let axis_len = (axis_end - axis_start).length();

    // Find closest points between ray and axis line
    let w0 = ray.origin - axis_start;
    let a = ray.direction.dot(*ray.direction);
    let b = ray.direction.dot(axis_dir);
    let c = axis_dir.dot(axis_dir);
    let d = ray.direction.dot(w0);
    let e = axis_dir.dot(w0);

    let denom = a * c - b * b;
    if denom.abs() < 0.0001 {
        return f32::MAX; // Lines are parallel
    }

    let t_ray = (b * e - c * d) / denom;
    let t_axis = (a * e - b * d) / denom;

    // Clamp t_axis to the segment
    let t_axis = t_axis.clamp(0.0, axis_len);
    let t_ray = t_ray.max(0.0);

    let closest_on_ray = ray.origin + *ray.direction * t_ray;
    let closest_on_axis = axis_start + axis_dir * t_axis;

    (closest_on_ray - closest_on_axis).length()
}

/// Find closest point on an axis line to a ray (for precise axis dragging)
pub fn ray_to_axis_closest_point(ray: &Ray3d, axis_origin: Vec3, axis_dir: Vec3) -> Vec3 {
    let w0 = ray.origin - axis_origin;
    let a = ray.direction.dot(*ray.direction);
    let b = ray.direction.dot(axis_dir);
    let c = axis_dir.dot(axis_dir);
    let d = ray.direction.dot(w0);
    let e = axis_dir.dot(w0);

    let denom = a * c - b * b;
    if denom.abs() < 0.0001 {
        return axis_origin; // Parallel
    }

    let t_axis = (a * e - b * d) / denom;
    axis_origin + axis_dir * t_axis
}

/// Find intersection point of ray with a plane
pub fn ray_plane_intersection(ray: &Ray3d, plane_point: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let denom = plane_normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return None; // Parallel
    }
    let t = (plane_point - ray.origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return None; // Behind
    }
    Some(ray.origin + *ray.direction * t)
}

/// Check if ray intersects a quad (for plane handles)
pub fn ray_quad_intersection(ray: &Ray3d, center: Vec3, normal: Vec3, half_size: f32) -> Option<f32> {
    let denom = normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return None; // Ray parallel to plane
    }

    let t = (center - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None; // Behind ray
    }

    let hit_point = ray.origin + *ray.direction * t;
    let local = hit_point - center;

    // Check if within quad bounds (simplified axis-aligned check)
    let (u_axis, v_axis) = if normal.abs().y > 0.9 {
        (Vec3::X, Vec3::Z) // XZ plane
    } else if normal.abs().x > 0.9 {
        (Vec3::Y, Vec3::Z) // YZ plane
    } else {
        (Vec3::X, Vec3::Y) // XY plane
    };

    let u = local.dot(u_axis).abs();
    let v = local.dot(v_axis).abs();

    if u <= half_size && v <= half_size {
        Some(t)
    } else {
        None
    }
}

/// Distance from ray to a circle (for rotation gizmo picking)
pub fn ray_to_circle_distance(ray: &Ray3d, center: Vec3, axis: Vec3, radius: f32) -> f32 {
    // Find where ray intersects the plane containing the circle
    if let Some(point) = ray_plane_intersection(ray, center, axis) {
        // Calculate distance from point to the circle
        let to_point = point - center;
        let dist_from_center = to_point.length();
        // Distance to the circle is |distance_from_center - radius|
        (dist_from_center - radius).abs()
    } else {
        f32::MAX
    }
}

/// Get the angle on a circle where the ray intersects the plane (for rotation dragging)
pub fn ray_circle_intersection_point(ray: &Ray3d, center: Vec3, axis: Vec3) -> Option<Vec3> {
    // Find where ray intersects the plane containing the circle
    ray_plane_intersection(ray, center, axis)
}

/// Check if ray intersects a box (for center cube)
pub fn ray_box_intersection(ray: &Ray3d, center: Vec3, half_size: f32) -> Option<f32> {
    let min = center - Vec3::splat(half_size);
    let max = center + Vec3::splat(half_size);

    let t1 = (min - ray.origin) / *ray.direction;
    let t2 = (max - ray.origin) / *ray.direction;

    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);

    if t_enter <= t_exit && t_exit > 0.0 {
        Some(if t_enter > 0.0 { t_enter } else { t_exit })
    } else {
        None
    }
}
