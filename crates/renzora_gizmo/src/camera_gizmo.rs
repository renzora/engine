//! Camera gizmo — draws a camera body, lens, and frustum wireframe
//! when a selected entity has a `Camera3d` component.

use bevy::prelude::*;

use renzora_editor::{EditorSelection, EditorCamera};

use crate::OverlayGizmoGroup;

pub fn draw_camera_gizmo(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    transform_q: Query<(&Transform, Option<&Projection>), (With<Camera3d>, Without<EditorCamera>)>,
) {
    let Some(selected) = selection.get() else { return };
    let Ok((transform, projection)) = transform_q.get(selected) else { return };

    let pos = transform.translation;
    let rotation = transform.rotation;

    let camera_color = Color::srgb(0.8, 0.8, 0.9);
    let frustum_color = Color::srgba(0.6, 0.7, 1.0, 0.7);

    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;

    // ── Camera body (box) ──────────────────────────────────────────────
    let body_length = 0.4;
    let body_width = 0.3;
    let body_height = 0.2;
    let body_center = pos - forward * (body_length * 0.5);

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

    // ── Lens (cylinder approximation) ──────────────────────────────────
    let lens_center = pos;
    let lens_radius = 0.12;
    let lens_depth = 0.1;
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

    // ── Frustum wireframe ──────────────────────────────────────────────
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

    // Near plane
    gizmos.line(near_bl, near_br, frustum_color);
    gizmos.line(near_br, near_tr, frustum_color);
    gizmos.line(near_tr, near_tl, frustum_color);
    gizmos.line(near_tl, near_bl, frustum_color);

    // Far plane
    gizmos.line(far_bl, far_br, frustum_color);
    gizmos.line(far_br, far_tr, frustum_color);
    gizmos.line(far_tr, far_tl, frustum_color);
    gizmos.line(far_tl, far_bl, frustum_color);

    // Connecting edges
    gizmos.line(near_bl, far_bl, frustum_color);
    gizmos.line(near_br, far_br, frustum_color);
    gizmos.line(near_tl, far_tl, frustum_color);
    gizmos.line(near_tr, far_tr, frustum_color);

    // ── Forward direction arrow ────────────────────────────────────────
    let arrow_color = Color::srgb(0.3, 0.6, 1.0);
    let arrow_start = pos + forward * 0.3;
    let arrow_end = pos + forward * 0.8;
    gizmos.line(arrow_start, arrow_end, arrow_color);
    gizmos.line(arrow_end, arrow_end - forward * 0.1 + right * 0.05, arrow_color);
    gizmos.line(arrow_end, arrow_end - forward * 0.1 - right * 0.05, arrow_color);
}

fn extract_projection_params(projection: Option<&Projection>) -> (f32, f32, f32, f32) {
    match projection {
        Some(Projection::Perspective(p)) => {
            (p.fov, 0.5, 3.0, p.aspect_ratio)
        }
        Some(Projection::Orthographic(_)) => {
            // Use a small FOV to approximate ortho as a box
            (0.1_f32, 0.5, 3.0, 16.0 / 9.0)
        }
        _ => {
            (45.0_f32.to_radians(), 0.5, 3.0, 16.0 / 9.0)
        }
    }
}
