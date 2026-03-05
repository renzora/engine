use bevy::prelude::*;

use renzora_runtime::EditorCamera;
use renzora_viewport::ViewportState;

/// Get ray from cursor position in viewport.
///
/// Maps window-space cursor coordinates to the render target's coordinate
/// space using the viewport panel's screen rect before calling `viewport_to_world`.
pub fn get_cursor_ray(
    viewport: &ViewportState,
    windows: &Query<&Window>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) -> Option<Ray3d> {
    let window = windows.single().ok()?;
    let cursor_pos = window.cursor_position()?;
    let _image_handle = viewport.image_handle.as_ref()?;

    // Map window cursor position to viewport-local coordinates
    let local_pos = cursor_pos - viewport.screen_position;

    // Check bounds
    if local_pos.x < 0.0 || local_pos.y < 0.0
        || local_pos.x > viewport.screen_size.x
        || local_pos.y > viewport.screen_size.y
    {
        return None;
    }

    // Scale to render target resolution
    let scale_x = viewport.current_size.x as f32 / viewport.screen_size.x.max(1.0);
    let scale_y = viewport.current_size.y as f32 / viewport.screen_size.y.max(1.0);
    let render_target_pos = Vec2::new(local_pos.x * scale_x, local_pos.y * scale_y);

    let (camera, camera_transform) = camera_query.single().ok()?;
    camera.viewport_to_world(camera_transform, render_target_pos).ok()
}

/// Distance from ray to line segment (for gizmo axis picking)
pub fn ray_to_axis_distance(ray: &Ray3d, axis_start: Vec3, axis_end: Vec3) -> f32 {
    let axis_dir = (axis_end - axis_start).normalize();
    let axis_len = (axis_end - axis_start).length();

    let w0 = ray.origin - axis_start;
    let a = ray.direction.dot(*ray.direction);
    let b = ray.direction.dot(axis_dir);
    let c = axis_dir.dot(axis_dir);
    let d = ray.direction.dot(w0);
    let e = axis_dir.dot(w0);

    let denom = a * c - b * b;
    if denom.abs() < 0.0001 {
        return f32::MAX;
    }

    let t_ray = (b * e - c * d) / denom;
    let t_axis = (a * e - b * d) / denom;

    let t_axis = t_axis.clamp(0.0, axis_len);
    let t_ray = t_ray.max(0.0);

    let closest_on_ray = ray.origin + *ray.direction * t_ray;
    let closest_on_axis = axis_start + axis_dir * t_axis;

    (closest_on_ray - closest_on_axis).length()
}

/// Find closest point on an axis line to a ray
pub fn ray_to_axis_closest_point(ray: &Ray3d, axis_origin: Vec3, axis_dir: Vec3) -> Vec3 {
    let w0 = ray.origin - axis_origin;
    let a = ray.direction.dot(*ray.direction);
    let b = ray.direction.dot(axis_dir);
    let c = axis_dir.dot(axis_dir);
    let d = ray.direction.dot(w0);
    let e = axis_dir.dot(w0);

    let denom = a * c - b * b;
    if denom.abs() < 0.0001 {
        return axis_origin;
    }

    let t_axis = (a * e - b * d) / denom;
    axis_origin + axis_dir * t_axis
}

/// Find intersection point of ray with a plane
pub fn ray_plane_intersection(ray: &Ray3d, plane_point: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let denom = plane_normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return None;
    }
    let t = (plane_point - ray.origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + *ray.direction * t)
}

/// Check if ray intersects a quad (for plane handles)
pub fn ray_quad_intersection(ray: &Ray3d, center: Vec3, normal: Vec3, half_size: f32) -> Option<f32> {
    let denom = normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return None;
    }

    let t = (center - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }

    let hit_point = ray.origin + *ray.direction * t;
    let local = hit_point - center;

    let (u_axis, v_axis) = if normal.abs().y > 0.9 {
        (Vec3::X, Vec3::Z)
    } else if normal.abs().x > 0.9 {
        (Vec3::Y, Vec3::Z)
    } else {
        (Vec3::X, Vec3::Y)
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
    if let Some(point) = ray_plane_intersection(ray, center, axis) {
        let to_point = point - center;
        let dist_from_center = to_point.length();
        (dist_from_center - radius).abs()
    } else {
        f32::MAX
    }
}

/// Get the intersection point on the plane of a circle
pub fn ray_circle_intersection_point(ray: &Ray3d, center: Vec3, axis: Vec3) -> Option<Vec3> {
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
