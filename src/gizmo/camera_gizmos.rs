//! Camera frustum gizmo visualization

use bevy::prelude::*;

use crate::core::{CameraDebugState, CameraProjectionType};
use crate::core::resources::camera_debug::CameraInfo;
use super::SelectionGizmoGroup;

/// Draw camera frustum gizmos for visualization
pub fn draw_camera_gizmos(
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    camera_debug: Res<CameraDebugState>,
    cameras: Query<(&Camera, &GlobalTransform, Option<&Projection>), With<Camera3d>>,
) {
    if !camera_debug.show_frustum_gizmos && !camera_debug.show_all_frustums && !camera_debug.show_camera_axes {
        return;
    }

    let frustum_color = camera_debug.frustum_color;

    for camera_info in &camera_debug.cameras {
        // Skip editor cameras
        if camera_info.is_editor_camera {
            continue;
        }

        // Check if we should draw this camera
        let should_draw_frustum = camera_debug.show_all_frustums
            || (camera_debug.show_frustum_gizmos && camera_debug.selected_camera == Some(camera_info.entity));

        let should_draw_axes = camera_debug.show_camera_axes
            && camera_debug.selected_camera == Some(camera_info.entity);

        if !should_draw_frustum && !should_draw_axes {
            continue;
        }

        // Get the actual camera transform
        let Ok((camera, transform, projection)) = cameras.get(camera_info.entity) else {
            continue;
        };

        let position = transform.translation();
        let rotation = transform.to_scale_rotation_translation().1;

        // Draw camera axes
        if should_draw_axes {
            let axis_length = 1.0;
            let right = rotation * Vec3::X;
            let up = rotation * Vec3::Y;
            let forward = rotation * -Vec3::Z;

            gizmos.line(position, position + right * axis_length, Color::srgb(1.0, 0.2, 0.2));
            gizmos.line(position, position + up * axis_length, Color::srgb(0.2, 1.0, 0.2));
            gizmos.line(position, position + forward * axis_length, Color::srgb(0.2, 0.2, 1.0));
        }

        // Draw frustum
        if should_draw_frustum {
            draw_frustum_for_camera(&mut gizmos, camera_info, position, rotation, frustum_color);
        }
    }
}

/// Draw a frustum wireframe for a camera
fn draw_frustum_for_camera(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    camera_info: &CameraInfo,
    position: Vec3,
    rotation: Quat,
    color: Color,
) {
    let near = camera_info.near;
    let far = camera_info.far.min(50.0); // Clamp far plane for visualization

    match camera_info.projection_type {
        CameraProjectionType::Perspective => {
            let fov = camera_info.fov_degrees.unwrap_or(45.0).to_radians();
            let aspect = camera_info.aspect_ratio;

            // Calculate frustum corners
            let near_height = 2.0 * near * (fov / 2.0).tan();
            let near_width = near_height * aspect;

            let far_height = 2.0 * far * (fov / 2.0).tan();
            let far_width = far_height * aspect;

            // Near plane corners (in camera space)
            let nbl = Vec3::new(-near_width / 2.0, -near_height / 2.0, -near);
            let nbr = Vec3::new(near_width / 2.0, -near_height / 2.0, -near);
            let ntr = Vec3::new(near_width / 2.0, near_height / 2.0, -near);
            let ntl = Vec3::new(-near_width / 2.0, near_height / 2.0, -near);

            // Far plane corners (in camera space)
            let fbl = Vec3::new(-far_width / 2.0, -far_height / 2.0, -far);
            let fbr = Vec3::new(far_width / 2.0, -far_height / 2.0, -far);
            let ftr = Vec3::new(far_width / 2.0, far_height / 2.0, -far);
            let ftl = Vec3::new(-far_width / 2.0, far_height / 2.0, -far);

            // Transform to world space
            let nbl = position + rotation * nbl;
            let nbr = position + rotation * nbr;
            let ntr = position + rotation * ntr;
            let ntl = position + rotation * ntl;
            let fbl = position + rotation * fbl;
            let fbr = position + rotation * fbr;
            let ftr = position + rotation * ftr;
            let ftl = position + rotation * ftl;

            // Draw near plane
            gizmos.line(nbl, nbr, color);
            gizmos.line(nbr, ntr, color);
            gizmos.line(ntr, ntl, color);
            gizmos.line(ntl, nbl, color);

            // Draw far plane
            gizmos.line(fbl, fbr, color);
            gizmos.line(fbr, ftr, color);
            gizmos.line(ftr, ftl, color);
            gizmos.line(ftl, fbl, color);

            // Draw connecting edges
            gizmos.line(nbl, fbl, color);
            gizmos.line(nbr, fbr, color);
            gizmos.line(ntr, ftr, color);
            gizmos.line(ntl, ftl, color);
        }
        CameraProjectionType::Orthographic => {
            let scale = camera_info.ortho_scale.unwrap_or(1.0);
            let half_width = scale * camera_info.aspect_ratio;
            let half_height = scale;

            // Near plane corners
            let nbl = Vec3::new(-half_width, -half_height, -near);
            let nbr = Vec3::new(half_width, -half_height, -near);
            let ntr = Vec3::new(half_width, half_height, -near);
            let ntl = Vec3::new(-half_width, half_height, -near);

            // Far plane corners
            let fbl = Vec3::new(-half_width, -half_height, -far);
            let fbr = Vec3::new(half_width, -half_height, -far);
            let ftr = Vec3::new(half_width, half_height, -far);
            let ftl = Vec3::new(-half_width, half_height, -far);

            // Transform to world space
            let nbl = position + rotation * nbl;
            let nbr = position + rotation * nbr;
            let ntr = position + rotation * ntr;
            let ntl = position + rotation * ntl;
            let fbl = position + rotation * fbl;
            let fbr = position + rotation * fbr;
            let ftr = position + rotation * ftr;
            let ftl = position + rotation * ftl;

            // Draw near plane
            gizmos.line(nbl, nbr, color);
            gizmos.line(nbr, ntr, color);
            gizmos.line(ntr, ntl, color);
            gizmos.line(ntl, nbl, color);

            // Draw far plane
            gizmos.line(fbl, fbr, color);
            gizmos.line(fbr, ftr, color);
            gizmos.line(ftr, ftl, color);
            gizmos.line(ftl, fbl, color);

            // Draw connecting edges
            gizmos.line(nbl, fbl, color);
            gizmos.line(nbr, fbr, color);
            gizmos.line(ntr, ftr, color);
            gizmos.line(ntl, ftl, color);
        }
    }
}
