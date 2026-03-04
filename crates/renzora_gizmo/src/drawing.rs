use bevy::prelude::*;
use bevy::math::Isometry3d;

use crate::modal_transform::ModalTransformState;
use crate::state::{DragAxis, EditorTool, GizmoMode, GizmoState, SnapTarget};
use crate::{SelectionGizmoGroup, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE};
use renzora_editor::EditorSelection;

pub fn draw_selection_gizmo(
    selection: Res<EditorSelection>,
    gizmo_state: Res<GizmoState>,
    modal: Res<ModalTransformState>,
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    transforms: Query<&Transform>,
) {
    // Don't draw gizmo during modal transform
    if modal.active {
        return;
    }

    let Some(selected) = selection.get() else {
        return;
    };

    let Ok(transform) = transforms.get(selected) else {
        return;
    };

    // Only draw in Transform tool mode
    if gizmo_state.tool != EditorTool::Transform {
        return;
    }

    let pos = transform.translation;

    let active_axis = gizmo_state.drag_axis.or(gizmo_state.hovered_axis);

    let highlight = Color::srgb(1.0, 1.0, 0.3);
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

    let gs = gizmo_state.gizmo_scale;
    let gizmo_size = GIZMO_SIZE * gs;

    match gizmo_state.mode {
        GizmoMode::Translate => {
            let xy_color = if active_axis == Some(DragAxis::XY) { highlight } else { Color::srgba(0.9, 0.9, 0.2, 0.9) };
            let xz_color = if active_axis == Some(DragAxis::XZ) { highlight } else { Color::srgba(0.9, 0.2, 0.9, 0.9) };
            let yz_color = if active_axis == Some(DragAxis::YZ) { highlight } else { Color::srgba(0.2, 0.9, 0.9, 0.9) };

            let plane_half = GIZMO_PLANE_SIZE * gs * 0.5;

            // XY plane handle
            let xy_center = pos + Vec3::new(GIZMO_PLANE_OFFSET * gs, GIZMO_PLANE_OFFSET * gs, 0.0);
            gizmos.line(xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_color);

            // XZ plane handle
            let xz_center = pos + Vec3::new(GIZMO_PLANE_OFFSET * gs, 0.0, GIZMO_PLANE_OFFSET * gs);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_color);

            // YZ plane handle
            let yz_center = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET * gs, GIZMO_PLANE_OFFSET * gs);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_color);
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            let x_iso = Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
            gizmos.circle(x_iso, radius, x_color);
            let y_iso = Isometry3d::new(pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
            gizmos.circle(y_iso, radius, y_color);
            let z_iso = Isometry3d::new(pos, Quat::IDENTITY);
            gizmos.circle(z_iso, radius, z_color);
        }
        GizmoMode::Scale => {
            gizmos.line(pos, pos + Vec3::X * gizmo_size, x_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::X * gizmo_size).with_scale(Vec3::splat(0.15 * gs)), x_color);

            gizmos.line(pos, pos + Vec3::Y * gizmo_size, y_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::Y * gizmo_size).with_scale(Vec3::splat(0.15 * gs)), y_color);

            gizmos.line(pos, pos + Vec3::Z * gizmo_size, z_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::Z * gizmo_size).with_scale(Vec3::splat(0.15 * gs)), z_color);
        }
    }

    // Draw snap indicator when snapping is active
    if gizmo_state.is_dragging {
        draw_snap_indicator(&mut gizmos, &gizmo_state, pos);
    }
}

fn draw_snap_indicator(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    gizmo_state: &GizmoState,
    current_pos: Vec3,
) {
    let snap_color = Color::srgb(0.0, 1.0, 0.5);
    let floor_color = Color::srgb(0.5, 0.8, 1.0);

    match gizmo_state.snap_target {
        SnapTarget::Entity(_) => {
            if let Some(target_pos) = gizmo_state.snap_target_position {
                gizmos.line(current_pos, target_pos, snap_color);

                let indicator_size = 0.2;
                gizmos.line(target_pos + Vec3::new(-indicator_size, 0.0, 0.0), target_pos + Vec3::new(indicator_size, 0.0, 0.0), snap_color);
                gizmos.line(target_pos + Vec3::new(0.0, -indicator_size, 0.0), target_pos + Vec3::new(0.0, indicator_size, 0.0), snap_color);
                gizmos.line(target_pos + Vec3::new(0.0, 0.0, -indicator_size), target_pos + Vec3::new(0.0, 0.0, indicator_size), snap_color);

                let iso = Isometry3d::new(target_pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
                gizmos.circle(iso, 0.15, snap_color);
            }
        }
        SnapTarget::Floor => {
            if let Some(floor_pos) = gizmo_state.snap_target_position {
                gizmos.line(current_pos, floor_pos, floor_color);

                let indicator_size = 0.3;
                gizmos.line(floor_pos + Vec3::new(-indicator_size, 0.0, 0.0), floor_pos + Vec3::new(indicator_size, 0.0, 0.0), floor_color);
                gizmos.line(floor_pos + Vec3::new(0.0, 0.0, -indicator_size), floor_pos + Vec3::new(0.0, 0.0, indicator_size), floor_color);

                let iso = Isometry3d::new(floor_pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
                gizmos.circle(iso, 0.2, floor_color);
            }
        }
        SnapTarget::None => {}
    }
}
