//! Unity-style rotation pie indicator.
//!
//! When the user is rotating an entity (either via the click-drag rotate ring
//! or the modal R-key flow), a filled wedge is painted on top of the
//! viewport showing the angle swept from drag-start to the current cursor.
//!
//! Rendered as an egui filled polygon — Bevy's immediate-mode gizmos can't
//! paint solid triangles, so a 3D mesh or a 2D-projected fill is the only
//! way to get a real Unity-style pie. A 2D overlay also reads consistently
//! regardless of viewing angle (the wedge is always coplanar to the rotation
//! plane in world space; projecting it to screen lands on the rotate ring
//! the user is dragging on).
//!
//! ## Architecture
//!
//! The egui overlay only has `&World`, so a Bevy `Update` system caches the
//! pie's world-space parameters into [`RotationPieState`] each frame, and
//! the overlay drawer projects that state into the viewport rect.
//!
//! Pivot/radius come from:
//! - **Click-drag rotate** — `GizmoState` (active_axis, drag_angle) +
//!   `GizmoRoot.translation` and `gizmo_state.gizmo_scale` for the ring.
//! - **Modal R** — `ModalTransformState` (axis_constraint, accumulated_delta,
//!   numeric_input) + average of selected `Transform.translation`s. Radius
//!   reuses `gizmo_state.gizmo_scale` (which keeps updating during modal).

use bevy::prelude::*;
use bevy_egui::egui;

use crate::modal_transform::{AxisConstraint, ModalTransformMode, ModalTransformState};
use crate::{GizmoAxis, GizmoMode, GizmoRoot, GizmoState, GIZMO_SIZE};

/// Cached pie state populated each frame by [`update_rotation_pie_state`].
/// Read by [`draw_pie_overlay`] which only has `&World`.
#[derive(Resource, Default, Clone, Copy)]
pub struct RotationPieState {
    pub active: bool,
    pub world_center: Vec3,
    pub axis: PieAxis,
    /// Signed angle in radians; positive follows the right-hand rule about `axis`.
    pub angle: f32,
    /// World-space radius of the wedge (matches the rotate ring).
    pub radius_world: f32,
}

#[derive(Default, Clone, Copy)]
pub enum PieAxis {
    X,
    Y,
    #[default]
    Z,
}

impl PieAxis {
    fn dirs(self) -> (Vec3, Vec3) {
        // (axis_dir, ref_dir at angle 0). Choosing references so a positive
        // angle sweeps in the natural direction for that axis.
        match self {
            PieAxis::X => (Vec3::X, Vec3::Y),
            PieAxis::Y => (Vec3::Y, Vec3::Z),
            PieAxis::Z => (Vec3::Z, Vec3::X),
        }
    }

    fn fill(self) -> egui::Color32 {
        match self {
            PieAxis::X => egui::Color32::from_rgba_premultiplied(255, 70, 70, 70),
            PieAxis::Y => egui::Color32::from_rgba_premultiplied(80, 220, 80, 70),
            PieAxis::Z => egui::Color32::from_rgba_premultiplied(90, 130, 255, 70),
        }
    }

    fn edge(self) -> egui::Color32 {
        // Bright yellow edges, matching the active-axis highlight style.
        egui::Color32::from_rgba_premultiplied(255, 220, 80, 255)
    }
}

fn gizmo_axis_to_pie(axis: GizmoAxis) -> Option<PieAxis> {
    match axis {
        GizmoAxis::X => Some(PieAxis::X),
        GizmoAxis::Y => Some(PieAxis::Y),
        GizmoAxis::Z => Some(PieAxis::Z),
        _ => None,
    }
}

fn axis_constraint_to_pie(c: AxisConstraint) -> PieAxis {
    match c {
        AxisConstraint::X | AxisConstraint::PlaneYZ => PieAxis::X,
        AxisConstraint::Y | AxisConstraint::PlaneXZ => PieAxis::Y,
        // `apply_rotate` uses world Z for None and Z-plane.
        _ => PieAxis::Z,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_rotation_pie_state(
    mut state: ResMut<RotationPieState>,
    mode: Res<GizmoMode>,
    gizmo_state: Res<GizmoState>,
    modal: Res<ModalTransformState>,
    gizmo_root: Query<&GlobalTransform, With<GizmoRoot>>,
    transforms: Query<&Transform>,
) {
    state.active = false;

    // Click-drag rotate has priority over modal (they're mutually exclusive
    // in practice — the rotate ring isn't pickable while modal is active —
    // but be explicit).
    if matches!(*mode, GizmoMode::Rotate)
        && !modal.active
        && gizmo_state.active_axis.is_some()
        && gizmo_state.drag_angle.abs() > 1e-3
    {
        let Some(axis) = gizmo_axis_to_pie(gizmo_state.active_axis.unwrap()) else {
            return;
        };
        let Ok(root_gt) = gizmo_root.single() else { return };
        let radius = (GIZMO_SIZE * gizmo_state.gizmo_scale * 0.7).max(0.05);
        state.active = true;
        state.world_center = root_gt.translation();
        state.axis = axis;
        state.angle = gizmo_state.drag_angle;
        state.radius_world = radius;
        return;
    }

    if modal.active && matches!(modal.mode, Some(ModalTransformMode::Rotate)) {
        let mut sum = Vec3::ZERO;
        let mut n = 0u32;
        for s in &modal.start_transforms {
            if let Ok(t) = transforms.get(s.entity) {
                sum += t.translation;
                n += 1;
            }
        }
        if n == 0 {
            return;
        }
        let pivot = sum / n as f32;
        let angle = if let Some(degrees) = modal.numeric_input.value() {
            degrees.to_radians()
        } else {
            let d = modal.accumulated_delta;
            (-d.x + d.y) * modal.sensitivity * 0.5
        };
        if angle.abs() <= 1e-3 {
            return;
        }
        let radius = (GIZMO_SIZE * gizmo_state.gizmo_scale * 0.7).max(0.05);
        state.active = true;
        state.world_center = pivot;
        state.axis = axis_constraint_to_pie(modal.axis_constraint);
        state.angle = angle;
        state.radius_world = radius;
    }
}

/// Egui overlay drawer registered with `ViewportOverlayRegistry`. Reads
/// [`RotationPieState`] and projects the wedge onto the viewport rect.
pub fn draw_pie_overlay(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let Some(state) = world.get_resource::<RotationPieState>() else { return };
    if !state.active {
        return;
    }
    let Some(cam_entity) = find_editor_camera(world) else { return };
    let Some(camera) = world.get::<Camera>(cam_entity) else { return };
    let Some(cam_gt) = world.get::<GlobalTransform>(cam_entity) else { return };

    let painter = ui.painter_at(rect);

    let (axis_dir, ref_dir) = state.axis.dirs();
    let total = state.angle;
    let radius = state.radius_world;

    // Cap segment count so a multi-revolution drag doesn't generate
    // thousands of tris; keep enough for tiny angles to look smooth.
    let segments = ((total.abs() / std::f32::consts::TAU * 96.0) as usize).clamp(8, 256);

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

    let center_2d = match project(state.world_center) {
        Some(p) => p,
        None => return,
    };

    let fill = state.axis.fill();
    let edge = state.axis.edge();

    // Pre-compute screen-space arc points; reuse them for both the fill
    // triangle fan and the outline polyline.
    let mut arc_pts: Vec<egui::Pos2> = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let a = total * t;
        let world_pt = state.world_center + Quat::from_axis_angle(axis_dir, a) * ref_dir * radius;
        if let Some(p) = project(world_pt) {
            arc_pts.push(p);
        }
    }
    if arc_pts.len() < 2 {
        return;
    }

    // Triangle fan from center → consecutive arc points. Each tri is a
    // valid convex polygon (always 3 vertices), so this works for any
    // angle including >360°. egui blends overlapping fills.
    for w in arc_pts.windows(2) {
        painter.add(egui::Shape::convex_polygon(
            vec![center_2d, w[0], w[1]],
            fill,
            egui::Stroke::NONE,
        ));
    }

    // Outline: arc + start edge + end edge.
    painter.add(egui::Shape::line(
        arc_pts.clone(),
        egui::Stroke::new(1.5, edge),
    ));
    let start_pt = arc_pts[0];
    let end_pt = *arc_pts.last().unwrap();
    painter.line_segment([center_2d, start_pt], egui::Stroke::new(1.5, edge));
    painter.line_segment([center_2d, end_pt], egui::Stroke::new(1.5, edge));

    // Numeric readout near the cursor end so the user sees how many degrees
    // they've swept. Background pill so it stays legible over any scene.
    let label = format!("{:.1}°", total.to_degrees());
    let text_pos = end_pt + egui::vec2(8.0, -8.0);
    let galley = painter.layout_no_wrap(label, egui::FontId::proportional(13.0), egui::Color32::WHITE);
    let bg = egui::Rect::from_min_size(text_pos, galley.size()).expand2(egui::vec2(4.0, 2.0));
    painter.rect_filled(bg, 3.0, egui::Color32::from_rgba_premultiplied(20, 20, 25, 200));
    painter.galley(text_pos, galley, egui::Color32::WHITE);
}

fn find_editor_camera(world: &World) -> Option<Entity> {
    // Reuse the SceneIconCache lookup so we don't duplicate the scan. The
    // cache is populated by `update_scene_icon_cache` every frame in editor
    // mode, so this is reliable while the pie is active.
    world
        .get_resource::<crate::light_gizmo::SceneIconCache>()
        .and_then(|c| c.editor_camera)
}
