use bevy::prelude::*;
use bevy::math::Isometry3d;

use crate::core::{EditorEntity, SelectionState};
use crate::node_system::CameraNodeData;

use super::{DragAxis, GizmoMode, GizmoState, GIZMO_CENTER_SIZE, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE};

pub fn draw_selection_gizmo(
    selection: Res<SelectionState>,
    gizmo_state: Res<GizmoState>,
    mut gizmos: Gizmos,
    transforms: Query<&Transform, With<EditorEntity>>,
    cameras: Query<&CameraNodeData>,
) {
    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(transform) = transforms.get(selected) else {
        return;
    };

    // Check if this is a camera node - draw camera gizmo in addition to transform gizmo
    if let Ok(camera_data) = cameras.get(selected) {
        draw_camera_gizmo(&mut gizmos, transform, camera_data);
        // Continue to draw transform gizmo as well
    }

    let pos = transform.translation;
    let scale = transform.scale;
    let half = (scale * 0.6).max(Vec3::splat(0.3));

    // Draw selection box outline (centered on object) - skip for cameras
    let is_camera = cameras.get(selected).is_ok();
    if !is_camera {
        let box_color = Color::srgb(1.0, 0.6, 0.0);

        // Bottom square
        gizmos.line(pos + Vec3::new(-half.x, -half.y, -half.z), pos + Vec3::new(half.x, -half.y, -half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, -half.y, -half.z), pos + Vec3::new(half.x, -half.y, half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, -half.y, half.z), pos + Vec3::new(-half.x, -half.y, half.z), box_color);
        gizmos.line(pos + Vec3::new(-half.x, -half.y, half.z), pos + Vec3::new(-half.x, -half.y, -half.z), box_color);

        // Top square
        gizmos.line(pos + Vec3::new(-half.x, half.y, -half.z), pos + Vec3::new(half.x, half.y, -half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, half.y, -half.z), pos + Vec3::new(half.x, half.y, half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, half.y, half.z), pos + Vec3::new(-half.x, half.y, half.z), box_color);
        gizmos.line(pos + Vec3::new(-half.x, half.y, half.z), pos + Vec3::new(-half.x, half.y, -half.z), box_color);

        // Vertical lines
        gizmos.line(pos + Vec3::new(-half.x, -half.y, -half.z), pos + Vec3::new(-half.x, half.y, -half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, -half.y, -half.z), pos + Vec3::new(half.x, half.y, -half.z), box_color);
        gizmos.line(pos + Vec3::new(half.x, -half.y, half.z), pos + Vec3::new(half.x, half.y, half.z), box_color);
        gizmos.line(pos + Vec3::new(-half.x, -half.y, half.z), pos + Vec3::new(-half.x, half.y, half.z), box_color);
    }

    // Determine axis colors based on hover/drag state
    let active_axis = gizmo_state.drag_axis.or(gizmo_state.hovered_axis);

    let highlight = Color::srgb(1.0, 1.0, 0.3); // Yellow highlight
    let x_base = Color::srgb(0.9, 0.2, 0.2);
    let y_base = Color::srgb(0.2, 0.9, 0.2);
    let z_base = Color::srgb(0.2, 0.2, 0.9);

    let x_color = if matches!(active_axis, Some(DragAxis::X) | Some(DragAxis::XY) | Some(DragAxis::XZ)) {
        highlight
    } else {
        x_base
    };

    let y_color = if matches!(active_axis, Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ)) {
        highlight
    } else {
        y_base
    };

    let z_color = if matches!(active_axis, Some(DragAxis::Z) | Some(DragAxis::XZ) | Some(DragAxis::YZ)) {
        highlight
    } else {
        z_base
    };

    let center_color = if active_axis == Some(DragAxis::Free) {
        highlight
    } else {
        Color::srgb(0.8, 0.8, 0.8)
    };

    let gizmo_size = GIZMO_SIZE;

    match gizmo_state.mode {
        GizmoMode::Translate => {
            // Helper to draw a thick line (multiple parallel lines)
            let draw_thick_axis = |gizmos: &mut Gizmos, start: Vec3, end: Vec3, color: Color| {
                let dir = (end - start).normalize();
                let thickness = 0.02;

                // Find perpendicular vectors
                let perp1 = if dir.y.abs() < 0.9 {
                    dir.cross(Vec3::Y).normalize()
                } else {
                    dir.cross(Vec3::X).normalize()
                };
                let perp2 = dir.cross(perp1).normalize();

                // Draw main line and offset lines for thickness
                gizmos.line(start, end, color);
                gizmos.line(start + perp1 * thickness, end + perp1 * thickness, color);
                gizmos.line(start - perp1 * thickness, end - perp1 * thickness, color);
                gizmos.line(start + perp2 * thickness, end + perp2 * thickness, color);
                gizmos.line(start - perp2 * thickness, end - perp2 * thickness, color);
            };

            // Helper to draw cone arrow head
            let draw_cone = |gizmos: &mut Gizmos, tip: Vec3, dir: Vec3, color: Color| {
                let cone_length = 0.25;
                let cone_radius = 0.08;
                let base = tip - dir * cone_length;

                let perp1 = if dir.y.abs() < 0.9 {
                    dir.cross(Vec3::Y).normalize()
                } else {
                    dir.cross(Vec3::X).normalize()
                };
                let perp2 = dir.cross(perp1).normalize();

                // Draw cone lines
                let segments = 8;
                for i in 0..segments {
                    let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                    let next_angle = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                    let p1 = base + (perp1 * angle.cos() + perp2 * angle.sin()) * cone_radius;
                    let p2 = base + (perp1 * next_angle.cos() + perp2 * next_angle.sin()) * cone_radius;

                    gizmos.line(tip, p1, color);
                    gizmos.line(p1, p2, color);
                }
            };

            // X axis
            draw_thick_axis(&mut gizmos, pos, pos + Vec3::X * (gizmo_size - 0.25), x_color);
            draw_cone(&mut gizmos, pos + Vec3::X * gizmo_size, Vec3::X, x_color);

            // Y axis
            draw_thick_axis(&mut gizmos, pos, pos + Vec3::Y * (gizmo_size - 0.25), y_color);
            draw_cone(&mut gizmos, pos + Vec3::Y * gizmo_size, Vec3::Y, y_color);

            // Z axis
            draw_thick_axis(&mut gizmos, pos, pos + Vec3::Z * (gizmo_size - 0.25), z_color);
            draw_cone(&mut gizmos, pos + Vec3::Z * gizmo_size, Vec3::Z, z_color);

            // Center cube
            gizmos.cuboid(
                Transform::from_translation(pos).with_scale(Vec3::splat(GIZMO_CENTER_SIZE * 2.0)),
                center_color,
            );

            // Plane handles (small squares)
            let xy_color = if active_axis == Some(DragAxis::XY) { highlight } else { Color::srgba(0.9, 0.9, 0.2, 0.6) };
            let xz_color = if active_axis == Some(DragAxis::XZ) { highlight } else { Color::srgba(0.9, 0.2, 0.9, 0.6) };
            let yz_color = if active_axis == Some(DragAxis::YZ) { highlight } else { Color::srgba(0.2, 0.9, 0.9, 0.6) };

            let plane_half = GIZMO_PLANE_SIZE * 0.5;

            // XY plane handle
            let xy_center = pos + Vec3::new(GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET, 0.0);
            gizmos.line(xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_color);
            // Fill lines
            gizmos.line(xy_center + Vec3::new(-plane_half, 0.0, 0.0), xy_center + Vec3::new(plane_half, 0.0, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(0.0, -plane_half, 0.0), xy_center + Vec3::new(0.0, plane_half, 0.0), xy_color);

            // XZ plane handle
            let xz_center = pos + Vec3::new(GIZMO_PLANE_OFFSET, 0.0, GIZMO_PLANE_OFFSET);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, 0.0), xz_center + Vec3::new(plane_half, 0.0, 0.0), xz_color);
            gizmos.line(xz_center + Vec3::new(0.0, 0.0, -plane_half), xz_center + Vec3::new(0.0, 0.0, plane_half), xz_color);

            // YZ plane handle
            let yz_center = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, 0.0), yz_center + Vec3::new(0.0, plane_half, 0.0), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, 0.0, -plane_half), yz_center + Vec3::new(0.0, 0.0, plane_half), yz_color);
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            // X axis circle (YZ plane)
            let x_iso = Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
            gizmos.circle(x_iso, radius, x_color);
            // Y axis circle (XZ plane)
            let y_iso = Isometry3d::new(pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
            gizmos.circle(y_iso, radius, y_color);
            // Z axis circle (XY plane)
            let z_iso = Isometry3d::new(pos, Quat::IDENTITY);
            gizmos.circle(z_iso, radius, z_color);
        }
        GizmoMode::Scale => {
            // X axis with box
            gizmos.line(pos, pos + Vec3::X * gizmo_size, x_color);
            gizmos.cuboid(Transform::from_translation(pos + Vec3::X * gizmo_size).with_scale(Vec3::splat(0.15)), x_color);

            // Y axis with box
            gizmos.line(pos, pos + Vec3::Y * gizmo_size, y_color);
            gizmos.cuboid(Transform::from_translation(pos + Vec3::Y * gizmo_size).with_scale(Vec3::splat(0.15)), y_color);

            // Z axis with box
            gizmos.line(pos, pos + Vec3::Z * gizmo_size, z_color);
            gizmos.cuboid(Transform::from_translation(pos + Vec3::Z * gizmo_size).with_scale(Vec3::splat(0.15)), z_color);
        }
    }
}

/// Draw a camera frustum gizmo for camera nodes
fn draw_camera_gizmo(gizmos: &mut Gizmos, transform: &Transform, camera_data: &CameraNodeData) {
    let pos = transform.translation;
    let rotation = transform.rotation;

    // Camera body color
    let camera_color = Color::srgb(0.8, 0.8, 0.9);
    let frustum_color = Color::srgba(0.6, 0.7, 1.0, 0.7);

    // Get local directions
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;

    // Camera body dimensions
    let body_length = 0.4;
    let body_width = 0.3;
    let body_height = 0.2;

    // Draw camera body (box shape)
    let body_center = pos - forward * (body_length * 0.5);

    // Body corners
    let blf = body_center - right * body_width * 0.5 - up * body_height * 0.5 - forward * body_length * 0.5;
    let brf = body_center + right * body_width * 0.5 - up * body_height * 0.5 - forward * body_length * 0.5;
    let tlf = body_center - right * body_width * 0.5 + up * body_height * 0.5 - forward * body_length * 0.5;
    let trf = body_center + right * body_width * 0.5 + up * body_height * 0.5 - forward * body_length * 0.5;
    let blb = body_center - right * body_width * 0.5 - up * body_height * 0.5 + forward * body_length * 0.5;
    let brb = body_center + right * body_width * 0.5 - up * body_height * 0.5 + forward * body_length * 0.5;
    let tlb = body_center - right * body_width * 0.5 + up * body_height * 0.5 + forward * body_length * 0.5;
    let trb = body_center + right * body_width * 0.5 + up * body_height * 0.5 + forward * body_length * 0.5;

    // Front face
    gizmos.line(blf, brf, camera_color);
    gizmos.line(brf, trf, camera_color);
    gizmos.line(trf, tlf, camera_color);
    gizmos.line(tlf, blf, camera_color);

    // Back face
    gizmos.line(blb, brb, camera_color);
    gizmos.line(brb, trb, camera_color);
    gizmos.line(trb, tlb, camera_color);
    gizmos.line(tlb, blb, camera_color);

    // Connecting edges
    gizmos.line(blf, blb, camera_color);
    gizmos.line(brf, brb, camera_color);
    gizmos.line(tlf, tlb, camera_color);
    gizmos.line(trf, trb, camera_color);

    // Draw lens (cylinder-like shape on front)
    let lens_center = pos;
    let lens_radius = 0.12;
    let lens_depth = 0.1;

    // Draw lens circles
    let segments = 12;
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let next_angle = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p1 = lens_center + (right * angle.cos() + up * angle.sin()) * lens_radius;
        let p2 = lens_center + (right * next_angle.cos() + up * next_angle.sin()) * lens_radius;
        let p3 = lens_center + forward * lens_depth + (right * angle.cos() + up * angle.sin()) * lens_radius;
        let p4 = lens_center + forward * lens_depth + (right * next_angle.cos() + up * next_angle.sin()) * lens_radius;

        gizmos.line(p1, p2, camera_color);
        gizmos.line(p3, p4, camera_color);
        if i % 3 == 0 {
            gizmos.line(p1, p3, camera_color);
        }
    }

    // Draw view frustum
    let fov_rad = camera_data.fov.to_radians();
    let near = 0.5;
    let far = 3.0;
    let aspect = 16.0 / 9.0;

    let near_height = near * (fov_rad / 2.0).tan();
    let near_width = near_height * aspect;
    let far_height = far * (fov_rad / 2.0).tan();
    let far_width = far_height * aspect;

    // Near plane corners
    let near_center = pos + forward * near;
    let near_bl = near_center - right * near_width - up * near_height;
    let near_br = near_center + right * near_width - up * near_height;
    let near_tl = near_center - right * near_width + up * near_height;
    let near_tr = near_center + right * near_width + up * near_height;

    // Far plane corners
    let far_center = pos + forward * far;
    let far_bl = far_center - right * far_width - up * far_height;
    let far_br = far_center + right * far_width - up * far_height;
    let far_tl = far_center - right * far_width + up * far_height;
    let far_tr = far_center + right * far_width + up * far_height;

    // Draw near plane
    gizmos.line(near_bl, near_br, frustum_color);
    gizmos.line(near_br, near_tr, frustum_color);
    gizmos.line(near_tr, near_tl, frustum_color);
    gizmos.line(near_tl, near_bl, frustum_color);

    // Draw far plane
    gizmos.line(far_bl, far_br, frustum_color);
    gizmos.line(far_br, far_tr, frustum_color);
    gizmos.line(far_tr, far_tl, frustum_color);
    gizmos.line(far_tl, far_bl, frustum_color);

    // Draw frustum edges (connecting near to far)
    gizmos.line(near_bl, far_bl, frustum_color);
    gizmos.line(near_br, far_br, frustum_color);
    gizmos.line(near_tl, far_tl, frustum_color);
    gizmos.line(near_tr, far_tr, frustum_color);

    // Draw forward direction indicator
    let arrow_start = pos + forward * 0.3;
    let arrow_end = pos + forward * 0.8;
    gizmos.line(arrow_start, arrow_end, Color::srgb(0.3, 0.6, 1.0));

    // Small arrow head
    let arrow_head_size = 0.1;
    gizmos.line(arrow_end, arrow_end - forward * arrow_head_size + right * arrow_head_size * 0.5, Color::srgb(0.3, 0.6, 1.0));
    gizmos.line(arrow_end, arrow_end - forward * arrow_head_size - right * arrow_head_size * 0.5, Color::srgb(0.3, 0.6, 1.0));
}
