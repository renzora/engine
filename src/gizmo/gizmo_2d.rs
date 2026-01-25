//! 2D gizmo rendering for the editor
//!
//! Draws translation, rotation, and scale gizmos for 2D entities.

use bevy::prelude::*;

use crate::core::{EditorEntity, SelectionState, ViewportMode, ViewportState};
use crate::viewport::Camera2DState;
use super::{DragAxis, GizmoMode, SelectionGizmoGroup};
use super::state::GizmoState;

/// Size of the gizmo axis lines in world units
const GIZMO_2D_SIZE: f32 = 80.0;
/// Size of the center handle
const GIZMO_2D_CENTER_SIZE: f32 = 12.0;
/// Arrow head size
const GIZMO_2D_ARROW_SIZE: f32 = 10.0;
/// Threshold distance for picking gizmo handles
pub const GIZMO_2D_PICK_THRESHOLD: f32 = 15.0;

/// Draw the 2D selection gizmo for the selected entity
pub fn draw_selection_gizmo_2d(
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
    selection: Res<SelectionState>,
    gizmo_state: Res<GizmoState>,
    transforms: Query<&Transform, With<EditorEntity>>,
) {
    // Only in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(transform) = transforms.get(selected) else {
        return;
    };

    let pos = transform.translation;
    let scale_factor = 1.0 / camera2d_state.zoom; // Keep gizmo size constant regardless of zoom

    match gizmo_state.mode {
        GizmoMode::Translate => {
            draw_translate_gizmo_2d(&mut gizmos, pos, scale_factor, &gizmo_state);
        }
        GizmoMode::Rotate => {
            draw_rotate_gizmo_2d(&mut gizmos, pos, scale_factor, &gizmo_state);
        }
        GizmoMode::Scale => {
            draw_scale_gizmo_2d(&mut gizmos, pos, scale_factor, &gizmo_state);
        }
    }
}

fn draw_translate_gizmo_2d(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    pos: Vec3,
    scale: f32,
    gizmo_state: &GizmoState,
) {
    let size = GIZMO_2D_SIZE * scale;
    let arrow = GIZMO_2D_ARROW_SIZE * scale;
    let center_size = GIZMO_2D_CENTER_SIZE * scale;

    // Colors - brighten hovered axis
    let x_color = if gizmo_state.hovered_axis == Some(DragAxis::X) || gizmo_state.drag_axis == Some(DragAxis::X) {
        Color::srgb(1.0, 0.5, 0.5)
    } else {
        Color::srgb(0.9, 0.2, 0.2)
    };

    let y_color = if gizmo_state.hovered_axis == Some(DragAxis::Y) || gizmo_state.drag_axis == Some(DragAxis::Y) {
        Color::srgb(0.5, 1.0, 0.5)
    } else {
        Color::srgb(0.2, 0.9, 0.2)
    };

    let center_color = if gizmo_state.hovered_axis == Some(DragAxis::Free) || gizmo_state.drag_axis == Some(DragAxis::Free) {
        Color::srgb(1.0, 1.0, 0.5)
    } else {
        Color::srgb(0.9, 0.9, 0.2)
    };

    // X axis (red) - pointing right
    let x_end = pos + Vec3::X * size;
    gizmos.line(pos, x_end, x_color);
    // Arrow head
    gizmos.line(x_end, x_end + Vec3::new(-arrow, arrow * 0.5, 0.0), x_color);
    gizmos.line(x_end, x_end + Vec3::new(-arrow, -arrow * 0.5, 0.0), x_color);

    // Y axis (green) - pointing up
    let y_end = pos + Vec3::Y * size;
    gizmos.line(pos, y_end, y_color);
    // Arrow head
    gizmos.line(y_end, y_end + Vec3::new(arrow * 0.5, -arrow, 0.0), y_color);
    gizmos.line(y_end, y_end + Vec3::new(-arrow * 0.5, -arrow, 0.0), y_color);

    // Center square (yellow) for free movement
    let half = center_size * 0.5;
    gizmos.line(
        pos + Vec3::new(-half, -half, 0.0),
        pos + Vec3::new(half, -half, 0.0),
        center_color,
    );
    gizmos.line(
        pos + Vec3::new(half, -half, 0.0),
        pos + Vec3::new(half, half, 0.0),
        center_color,
    );
    gizmos.line(
        pos + Vec3::new(half, half, 0.0),
        pos + Vec3::new(-half, half, 0.0),
        center_color,
    );
    gizmos.line(
        pos + Vec3::new(-half, half, 0.0),
        pos + Vec3::new(-half, -half, 0.0),
        center_color,
    );
}

fn draw_rotate_gizmo_2d(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    pos: Vec3,
    scale: f32,
    gizmo_state: &GizmoState,
) {
    let radius = GIZMO_2D_SIZE * scale * 0.7;

    // Z axis rotation circle (blue - rotation around Z axis in 2D)
    let z_color = if gizmo_state.hovered_axis == Some(DragAxis::Z) || gizmo_state.drag_axis == Some(DragAxis::Z) {
        Color::srgb(0.5, 0.5, 1.0)
    } else {
        Color::srgb(0.2, 0.2, 0.9)
    };

    // Draw circle using line segments
    let segments = 32;
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p1 = pos + Vec3::new(angle1.cos() * radius, angle1.sin() * radius, 0.0);
        let p2 = pos + Vec3::new(angle2.cos() * radius, angle2.sin() * radius, 0.0);

        gizmos.line(p1, p2, z_color);
    }
}

fn draw_scale_gizmo_2d(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    pos: Vec3,
    scale: f32,
    gizmo_state: &GizmoState,
) {
    let size = GIZMO_2D_SIZE * scale;
    let box_size = GIZMO_2D_ARROW_SIZE * scale * 0.8;
    let center_size = GIZMO_2D_CENTER_SIZE * scale;

    // Colors
    let x_color = if gizmo_state.hovered_axis == Some(DragAxis::X) || gizmo_state.drag_axis == Some(DragAxis::X) {
        Color::srgb(1.0, 0.5, 0.5)
    } else {
        Color::srgb(0.9, 0.2, 0.2)
    };

    let y_color = if gizmo_state.hovered_axis == Some(DragAxis::Y) || gizmo_state.drag_axis == Some(DragAxis::Y) {
        Color::srgb(0.5, 1.0, 0.5)
    } else {
        Color::srgb(0.2, 0.9, 0.2)
    };

    let center_color = if gizmo_state.hovered_axis == Some(DragAxis::Free) || gizmo_state.drag_axis == Some(DragAxis::Free) {
        Color::srgb(1.0, 1.0, 0.5)
    } else {
        Color::srgb(0.9, 0.9, 0.2)
    };

    // X axis with box at end
    let x_end = pos + Vec3::X * size;
    gizmos.line(pos, x_end, x_color);
    // Box at end
    let half = box_size * 0.5;
    gizmos.line(x_end + Vec3::new(-half, -half, 0.0), x_end + Vec3::new(half, -half, 0.0), x_color);
    gizmos.line(x_end + Vec3::new(half, -half, 0.0), x_end + Vec3::new(half, half, 0.0), x_color);
    gizmos.line(x_end + Vec3::new(half, half, 0.0), x_end + Vec3::new(-half, half, 0.0), x_color);
    gizmos.line(x_end + Vec3::new(-half, half, 0.0), x_end + Vec3::new(-half, -half, 0.0), x_color);

    // Y axis with box at end
    let y_end = pos + Vec3::Y * size;
    gizmos.line(pos, y_end, y_color);
    // Box at end
    gizmos.line(y_end + Vec3::new(-half, -half, 0.0), y_end + Vec3::new(half, -half, 0.0), y_color);
    gizmos.line(y_end + Vec3::new(half, -half, 0.0), y_end + Vec3::new(half, half, 0.0), y_color);
    gizmos.line(y_end + Vec3::new(half, half, 0.0), y_end + Vec3::new(-half, half, 0.0), y_color);
    gizmos.line(y_end + Vec3::new(-half, half, 0.0), y_end + Vec3::new(-half, -half, 0.0), y_color);

    // Center square for uniform scale
    let c_half = center_size * 0.5;
    gizmos.line(pos + Vec3::new(-c_half, -c_half, 0.0), pos + Vec3::new(c_half, -c_half, 0.0), center_color);
    gizmos.line(pos + Vec3::new(c_half, -c_half, 0.0), pos + Vec3::new(c_half, c_half, 0.0), center_color);
    gizmos.line(pos + Vec3::new(c_half, c_half, 0.0), pos + Vec3::new(-c_half, c_half, 0.0), center_color);
    gizmos.line(pos + Vec3::new(-c_half, c_half, 0.0), pos + Vec3::new(-c_half, -c_half, 0.0), center_color);
}
