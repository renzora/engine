//! Which handle is under the cursor.
//!
//! The hit-test anchors on exactly the pivot, basis, scale and signs the draw
//! code used, because a grab area that does not coincide with the drawn handle
//! is a handle the user cannot hit.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState};
use renzora_editor_framework::{EditorCamera, EditorSelection};

use crate::modal_transform;
use crate::pivot::compute_gizmo_pivot;
use crate::ray::{
    closest_distance_ray_segment, pick_threshold, ray_circle_distance, ray_hits_plane_quad,
    viewport_cursor_ray,
};
use crate::types::{
    gizmo_basis, GizmoAxis, GizmoMode, GizmoSpace, GizmoState, AXES, PLANES,
};
use crate::{GIZMO_PLANE_SIZE, GIZMO_SIZE};

pub(crate) fn gizmo_hover_detect(
    mut gizmo_state: ResMut<GizmoState>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    transform_q: Query<&GlobalTransform, Without<EditorCamera>>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
    // The hit-test has to anchor exactly where the handles are drawn, or the
    // grab area sits somewhere the user cannot see.
    viewport_settings: Option<Res<ViewportSettings>>,
) {
    if modal.active {
        gizmo_state.hovered_axis = None;
        return;
    }
    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        gizmo_state.hovered_axis = None;
        return;
    }
    if gizmo_state.active_axis.is_some() {
        return;
    }
    gizmo_state.hovered_axis = None;

    let Some(selected) = selection.get() else {
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    if !viewport.hovered {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) || mouse_button.pressed(MouseButton::Middle) {
        return;
    }

    let Ok((cam_gt, projection)) = camera_q.single() else {
        return;
    };
    let Ok(entity_gt) = transform_q.get(selected) else {
        return;
    };
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(ray) = viewport_cursor_ray(window, viewport, cam_gt, projection) else {
        return;
    };

    let pivot_bottom = viewport_settings
        .as_ref()
        .map(|s| s.gizmo_pivot_bottom)
        .unwrap_or(true);
    let entity_pos = compute_gizmo_pivot(selected, &aabbs, &children_q, entity_gt, pivot_bottom);
    let gs = gizmo_state.gizmo_scale.max(0.01);
    let gizmo_size = GIZMO_SIZE * gs;
    let threshold = pick_threshold(cam_gt, entity_pos, projection, viewport.screen_size.y);
    // Same orientation the handles are drawn with, so picking matches visuals.
    let basis = gizmo_basis(*space, *mode, entity_gt.rotation());

    let mut best: Option<(GizmoAxis, f32)> = None;

    match *mode {
        GizmoMode::Select | GizmoMode::None => unreachable!(),
        GizmoMode::Translate => {
            // Plane squares first — inner corner at the origin, two edges along
            // the (signed, camera-facing) axes, matching `draw_line_gizmos`.
            let side = GIZMO_PLANE_SIZE * gs;
            for plane in PLANES {
                let (sa, sb) = plane.signed_plane_axes(gizmo_state.axis_signs).unwrap();
                let a = basis * sa;
                let b = basis * sb;
                if ray_hits_plane_quad(&ray, entity_pos, a, b, side) {
                    best = Some((plane, 0.0));
                    break;
                }
            }
            if best.is_none() {
                for axis in AXES {
                    let dir = basis * axis.signed_direction(gizmo_state.axis_signs);
                    if let Some(dist) = closest_distance_ray_segment(
                        &ray,
                        entity_pos,
                        entity_pos + dir * gizmo_size,
                    ) {
                        if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                            best = Some((axis, dist));
                        }
                    }
                }
            }
        }
        GizmoMode::Scale => {
            for axis in AXES {
                let dir = basis * axis.signed_direction(gizmo_state.axis_signs);
                if let Some(dist) =
                    closest_distance_ray_segment(&ray, entity_pos, entity_pos + dir * gizmo_size)
                {
                    if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            for axis in AXES {
                if let Some(dist) =
                    ray_circle_distance(&ray, entity_pos, basis * axis.direction(), radius)
                {
                    if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
    }

    gizmo_state.hovered_axis = best.map(|(a, _)| a);
}
