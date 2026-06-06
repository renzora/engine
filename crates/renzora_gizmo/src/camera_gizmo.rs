//! Camera gizmo.
//!
//! When a camera is selected, draw its frustum wireframe and a
//! forward-direction arrow as 3D immediate-mode gizmos.
//!
//! Note: the always-visible phosphor scene-icon overlay was dropped in the
//! egui purge and is pending a native re-implementation.

use bevy::prelude::*;

use renzora_editor::{EditorCamera, EditorSelection};

use crate::OverlayGizmoGroup;

// ── Selection-only 3D wireframe (frustum + forward arrow) ───────────────────

pub fn draw_camera_gizmo(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    transform_q: Query<
        (&GlobalTransform, Option<&Projection>),
        (With<Camera3d>, Without<EditorCamera>),
    >,
) {
    let Some(selected) = selection.get() else {
        return;
    };
    let Ok((gt, projection)) = transform_q.get(selected) else {
        return;
    };

    let pos = gt.translation();
    let rotation = gt.rotation();
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;

    let frustum_color = Color::srgba(0.6, 0.7, 1.0, 0.7);
    let (fov_rad, near_dist, far_dist, aspect) = extract_projection_params(projection);

    let near_height = near_dist * (fov_rad / 2.0).tan();
    let near_width = near_height * aspect;
    let far_height = far_dist * (fov_rad / 2.0).tan();
    let far_width = far_height * aspect;

    let near_center = pos + forward * near_dist;
    let near_bl = near_center - right * near_width - up * near_height;
    let near_br = near_center + right * near_width - up * near_height;
    let near_tl = near_center - right * near_width + up * near_height;
    let near_tr = near_center + right * near_width + up * near_height;

    let far_center = pos + forward * far_dist;
    let far_bl = far_center - right * far_width - up * far_height;
    let far_br = far_center + right * far_width - up * far_height;
    let far_tl = far_center - right * far_width + up * far_height;
    let far_tr = far_center + right * far_width + up * far_height;

    gizmos.line(near_bl, near_br, frustum_color);
    gizmos.line(near_br, near_tr, frustum_color);
    gizmos.line(near_tr, near_tl, frustum_color);
    gizmos.line(near_tl, near_bl, frustum_color);
    gizmos.line(far_bl, far_br, frustum_color);
    gizmos.line(far_br, far_tr, frustum_color);
    gizmos.line(far_tr, far_tl, frustum_color);
    gizmos.line(far_tl, far_bl, frustum_color);
    gizmos.line(near_bl, far_bl, frustum_color);
    gizmos.line(near_br, far_br, frustum_color);
    gizmos.line(near_tl, far_tl, frustum_color);
    gizmos.line(near_tr, far_tr, frustum_color);

    let arrow_color = Color::srgb(0.3, 0.6, 1.0);
    let arrow_start = pos + forward * 0.3;
    let arrow_end = pos + forward * 0.8;
    gizmos.line(arrow_start, arrow_end, arrow_color);
    gizmos.line(
        arrow_end,
        arrow_end - forward * 0.1 + right * 0.05,
        arrow_color,
    );
    gizmos.line(
        arrow_end,
        arrow_end - forward * 0.1 - right * 0.05,
        arrow_color,
    );
}

fn extract_projection_params(projection: Option<&Projection>) -> (f32, f32, f32, f32) {
    match projection {
        Some(Projection::Perspective(p)) => (p.fov, 0.5, 3.0, p.aspect_ratio),
        Some(Projection::Orthographic(_)) => (0.1_f32, 0.5, 3.0, 16.0 / 9.0),
        _ => (45.0_f32.to_radians(), 0.5, 3.0, 16.0 / 9.0),
    }
}
