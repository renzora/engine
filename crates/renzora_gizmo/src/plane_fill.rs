//! Filled plane handles for the translate gizmo.
//!
//! The XY / XZ / YZ plane drag handles are picked by ray-versus-quad math
//! (see `gizmo_hover_detect`) so picking doesn't care whether they're
//! drawn as outlines or filled polygons. The line gizmo group draws crisp
//! outlines but can't paint solid fills — egui can. This module caches the
//! four world-space corners of each plane handle each frame, and an egui
//! overlay drawer projects them and paints filled convex polygons.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::EditorSelection;

use crate::{
    compute_gizmo_pivot, GizmoAxis, GizmoMode, GizmoRoot, GizmoState,
    GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE,
};

/// Per-plane fill data. Corners are in world space.
#[derive(Clone, Copy)]
pub struct PlaneFill {
    pub axis: GizmoAxis,
    /// World-space corners of the quad in CCW order.
    pub corners: [Vec3; 4],
}

#[derive(Resource, Default)]
pub struct PlaneFillState {
    pub active: bool,
    pub planes: Vec<PlaneFill>,
}

#[allow(clippy::type_complexity)]
pub fn update_plane_fill_state(
    mut state: ResMut<PlaneFillState>,
    selection: Res<EditorSelection>,
    mode: Res<GizmoMode>,
    gizmo_state: Res<GizmoState>,
    modal: Res<crate::modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    gizmo_root: Query<&GlobalTransform, With<GizmoRoot>>,
    transforms: Query<&GlobalTransform, (Without<GizmoRoot>, Without<crate::GizmoMesh>)>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
) {
    state.planes.clear();
    state.active = false;

    if !matches!(*mode, GizmoMode::Translate) {
        return;
    }
    if modal.active {
        return;
    }
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        return;
    }
    let Some(selected) = selection.get() else { return };
    let Ok(sel_gt) = transforms.get(selected) else { return };
    let pivot = compute_gizmo_pivot(selected, &aabbs, &children_q, sel_gt);

    // Match the picking layout exactly: planes always sit in the +a+b
    // octant at offset GIZMO_PLANE_OFFSET * gs and span GIZMO_PLANE_SIZE * gs.
    // Anchored to the world-space pivot — independent of axis_signs, which
    // only flips single-axis arrows toward the camera.
    let gs = gizmo_root
        .single()
        .ok()
        .map(|gt| gt.scale().abs().max_element())
        .unwrap_or(gizmo_state.gizmo_scale.max(0.01));

    let half = GIZMO_PLANE_SIZE * gs * 0.5;
    let po = GIZMO_PLANE_OFFSET * gs;
    let signs = gizmo_state.axis_signs;

    for axis in [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ] {
        // Signed offsets so the plane handle sits in the same quadrant as
        // the (camera-facing) single-axis arrows. The unsigned `(a, b)` is
        // still the spanning basis for the quad itself — signs only shift
        // where its center lands.
        let (sa, sb) = match axis.signed_plane_axes(signs) {
            Some(ab) => ab,
            None => continue,
        };
        let (a, b) = match axis.plane_axes() {
            Some(ab) => ab,
            None => continue,
        };
        let center = pivot + sa * po + sb * po;
        let corners = [
            center - a * half - b * half,
            center + a * half - b * half,
            center + a * half + b * half,
            center - a * half + b * half,
        ];
        state.planes.push(PlaneFill { axis, corners });
    }
    state.active = !state.planes.is_empty();
}

pub fn draw_plane_fill_overlay(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let Some(state) = world.get_resource::<PlaneFillState>() else { return };
    if !state.active {
        return;
    }
    let Some(cam_entity) = world
        .get_resource::<crate::light_gizmo::SceneIconCache>()
        .and_then(|c| c.editor_camera)
    else {
        return;
    };
    let Some(camera) = world.get::<Camera>(cam_entity) else { return };
    let Some(cam_gt) = world.get::<GlobalTransform>(cam_entity) else { return };
    let Some(gizmo_state) = world.get_resource::<GizmoState>() else { return };

    let painter = ui.painter_at(rect);
    let active = gizmo_state.active_axis.or(gizmo_state.hovered_axis);

    let project = |world_pos: Vec3| -> Option<egui::Pos2> {
        let ndc = camera.world_to_ndc(cam_gt, world_pos)?;
        if !(0.0..=1.0).contains(&ndc.z) {
            return None;
        }
        Some(egui::pos2(
            rect.min.x + (ndc.x + 1.0) * 0.5 * rect.width(),
            rect.min.y + (1.0 - ndc.y) * 0.5 * rect.height(),
        ))
    };

    for plane in &state.planes {
        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(4);
        for c in plane.corners {
            if let Some(p) = project(c) {
                pts.push(p);
            }
        }
        if pts.len() < 3 {
            continue;
        }

        let is_active = active == Some(plane.axis);
        let (fill, stroke) = plane_colors(plane.axis, is_active);

        // Triangle fan from corner 0 — the quad is always convex so two
        // triangles are enough, but using `convex_polygon` directly works
        // too. Stroke handles the outline; remove the line-based outline
        // in `draw_line_gizmos` so we don't double-draw.
        painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
    }
}

fn plane_colors(axis: GizmoAxis, is_active: bool) -> (egui::Color32, egui::Stroke) {
    // Match the line-gizmo outline palette so the filled handles read the
    // same as before. Yellow/active highlight matches the single-axis
    // active color.
    let (r, g, b) = match axis {
        GizmoAxis::XY => (230, 230, 50),  // X+Y → yellow
        GizmoAxis::XZ => (230, 50, 230),  // X+Z → magenta
        GizmoAxis::YZ => (50, 230, 230),  // Y+Z → cyan
        _ => (200, 200, 200),
    };
    let (fill_a, stroke_a) = if is_active { (180, 255) } else { (90, 200) };
    let stroke_width = if is_active { 2.0 } else { 1.0 };
    let (sr, sg, sb) = if is_active { (255, 255, 80) } else { (r, g, b) };
    (
        egui::Color32::from_rgba_premultiplied(r, g, b, fill_a),
        egui::Stroke::new(
            stroke_width,
            egui::Color32::from_rgba_premultiplied(sr, sg, sb, stroke_a),
        ),
    )
}
