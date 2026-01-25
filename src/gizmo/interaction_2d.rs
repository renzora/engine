//! 2D gizmo interaction system
//!
//! Handles hovering and dragging of 2D gizmos for transform manipulation.

use bevy::prelude::*;

use crate::commands::{CommandHistory, SetTransformCommand, queue_command};
use crate::core::{EditorEntity, SceneManagerState, SelectionState, ViewportMode, ViewportState};
use crate::viewport::Camera2DState;
use super::gizmo_2d::GIZMO_2D_PICK_THRESHOLD;
use super::{DragAxis, GizmoMode, GizmoState};

/// Size constants matching gizmo_2d.rs
const GIZMO_2D_SIZE: f32 = 80.0;
const GIZMO_2D_CENTER_SIZE: f32 = 12.0;

/// System to detect hover over 2D gizmo handles
pub fn gizmo_2d_hover_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
    windows: Query<&Window>,
    transforms: Query<&Transform, With<EditorEntity>>,
) {
    // Only in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    // Don't update hover while dragging
    if gizmo.is_dragging {
        return;
    }

    gizmo.hovered_axis = None;

    if !viewport.hovered {
        return;
    }

    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(transform) = transforms.get(selected) else {
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Convert entity position to screen coordinates
    let entity_pos = Vec2::new(transform.translation.x, transform.translation.y);
    let screen_pos = world_to_screen_2d(entity_pos, &viewport, &camera2d_state);

    // Scale factor for screen-space calculations
    let scale = camera2d_state.zoom;

    match gizmo.mode {
        GizmoMode::Translate | GizmoMode::Scale => {
            let size = GIZMO_2D_SIZE * scale;
            let center_size = GIZMO_2D_CENTER_SIZE * scale;
            let threshold = GIZMO_2D_PICK_THRESHOLD * scale;

            // Check center handle first
            let center_dist = (cursor_pos - screen_pos).length();
            if center_dist < center_size {
                gizmo.hovered_axis = Some(DragAxis::Free);
                return;
            }

            // Check X axis
            let x_end = screen_pos + Vec2::X * size;
            let x_dist = distance_to_line_segment(cursor_pos, screen_pos, x_end);
            if x_dist < threshold {
                gizmo.hovered_axis = Some(DragAxis::X);
                return;
            }

            // Check Y axis (screen Y is inverted)
            let y_end = screen_pos - Vec2::Y * size; // Inverted for screen coords
            let y_dist = distance_to_line_segment(cursor_pos, screen_pos, y_end);
            if y_dist < threshold {
                gizmo.hovered_axis = Some(DragAxis::Y);
                return;
            }
        }
        GizmoMode::Rotate => {
            let radius = GIZMO_2D_SIZE * scale * 0.7;
            let threshold = GIZMO_2D_PICK_THRESHOLD * scale * 1.5;

            // Check distance to rotation circle
            let dist_from_center = (cursor_pos - screen_pos).length();
            let dist_to_circle = (dist_from_center - radius).abs();

            if dist_to_circle < threshold {
                gizmo.hovered_axis = Some(DragAxis::Z); // Z rotation in 2D
            }
        }
    }
}

/// System to handle 2D gizmo interaction (click and drag)
pub fn gizmo_2d_interaction_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    transforms: Query<&Transform, With<EditorEntity>>,
    mut command_history: ResMut<CommandHistory>,
) {
    // Only in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    // Handle drag end - create undo command
    if mouse_button.just_released(MouseButton::Left) {
        if gizmo.is_dragging {
            if let (Some(entity), Some(start_transform)) = (gizmo.drag_entity, gizmo.drag_start_transform) {
                if let Ok(current_transform) = transforms.get(entity) {
                    if *current_transform != start_transform {
                        let mut cmd = SetTransformCommand::new(entity, *current_transform);
                        cmd.old_transform = Some(start_transform);
                        queue_command(&mut command_history, Box::new(cmd));
                    }
                }
            }
        }
        gizmo.is_dragging = false;
        gizmo.drag_axis = None;
        gizmo.drag_start_transform = None;
        gizmo.drag_entity = None;
        return;
    }

    if !viewport.hovered {
        return;
    }

    // Only on click
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // If hovering over a gizmo axis, start drag
    if let Some(axis) = gizmo.hovered_axis {
        if let Some(selected) = selection.selected_entity {
            if let Ok(transform) = transforms.get(selected) {
                let Ok(window) = windows.single() else { return };
                let Some(cursor_pos) = window.cursor_position() else { return };

                // Store drag start state
                let world_cursor = screen_to_world_2d(cursor_pos, &viewport, &camera2d_state);
                let entity_pos = Vec2::new(transform.translation.x, transform.translation.y);

                gizmo.drag_start_offset = Vec3::new(
                    world_cursor.x - entity_pos.x,
                    world_cursor.y - entity_pos.y,
                    0.0,
                );

                gizmo.drag_start_transform = Some(*transform);
                gizmo.drag_entity = Some(selected);
                gizmo.drag_start_rotation = transform.rotation;
                gizmo.drag_start_scale = transform.scale;

                // For rotation, store initial angle
                if gizmo.mode == GizmoMode::Rotate {
                    let to_cursor = world_cursor - entity_pos;
                    gizmo.drag_start_angle = to_cursor.y.atan2(to_cursor.x);
                }

                // For scale, store initial distance
                if gizmo.mode == GizmoMode::Scale {
                    gizmo.drag_start_distance = (world_cursor - entity_pos).length().max(0.001);
                }

                gizmo.is_dragging = true;
                gizmo.drag_axis = Some(axis);
            }
        }
    }
}

/// System to handle 2D gizmo dragging
pub fn gizmo_2d_drag_system(
    gizmo: Res<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut transforms: Query<&mut Transform, With<EditorEntity>>,
    mut scene_state: ResMut<SceneManagerState>,
) {
    // Only in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    if !gizmo.is_dragging || !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    scene_state.mark_modified();

    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(mut transform) = transforms.get_mut(selected) else {
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let world_cursor = screen_to_world_2d(cursor_pos, &viewport, &camera2d_state);

    match gizmo.mode {
        GizmoMode::Translate => {
            let offset = Vec2::new(gizmo.drag_start_offset.x, gizmo.drag_start_offset.y);

            let new_pos = match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let new_x = world_cursor.x - offset.x;
                    Vec2::new(gizmo.snap.snap_translate(new_x), transform.translation.y)
                }
                Some(DragAxis::Y) => {
                    let new_y = world_cursor.y - offset.y;
                    Vec2::new(transform.translation.x, gizmo.snap.snap_translate(new_y))
                }
                Some(DragAxis::Free) => {
                    let new_pos = world_cursor - offset;
                    Vec2::new(
                        gizmo.snap.snap_translate(new_pos.x),
                        gizmo.snap.snap_translate(new_pos.y),
                    )
                }
                _ => Vec2::new(transform.translation.x, transform.translation.y),
            };

            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;
        }
        GizmoMode::Rotate => {
            if gizmo.drag_axis != Some(DragAxis::Z) {
                return;
            }

            let entity_pos = Vec2::new(transform.translation.x, transform.translation.y);
            let to_cursor = world_cursor - entity_pos;
            let current_angle = to_cursor.y.atan2(to_cursor.x);
            let delta_angle = current_angle - gizmo.drag_start_angle;

            let snapped_angle = gizmo.snap.snap_rotate(delta_angle);
            let rotation_delta = Quat::from_rotation_z(snapped_angle);
            transform.rotation = rotation_delta * gizmo.drag_start_rotation;
        }
        GizmoMode::Scale => {
            let entity_pos = Vec2::new(transform.translation.x, transform.translation.y);
            let current_dist = (world_cursor - entity_pos).length().max(0.001);
            let scale_factor = current_dist / gizmo.drag_start_distance;

            match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let new_scale = (gizmo.drag_start_scale.x * scale_factor).max(0.01);
                    transform.scale.x = gizmo.snap.snap_scale(new_scale);
                }
                Some(DragAxis::Y) => {
                    let new_scale = (gizmo.drag_start_scale.y * scale_factor).max(0.01);
                    transform.scale.y = gizmo.snap.snap_scale(new_scale);
                }
                Some(DragAxis::Free) => {
                    let new_scale = (gizmo.drag_start_scale * scale_factor).max(Vec3::splat(0.01));
                    transform.scale = gizmo.snap.snap_scale_vec3(new_scale);
                }
                _ => {}
            }
        }
    }
}

/// Convert screen coordinates to 2D world coordinates
fn screen_to_world_2d(screen_pos: Vec2, viewport: &ViewportState, camera2d_state: &Camera2DState) -> Vec2 {
    let viewport_center = Vec2::new(
        viewport.position[0] + viewport.size[0] / 2.0,
        viewport.position[1] + viewport.size[1] / 2.0,
    );

    let relative_pos = screen_pos - viewport_center;
    let world_x = relative_pos.x / camera2d_state.zoom + camera2d_state.pan_offset.x;
    let world_y = -relative_pos.y / camera2d_state.zoom + camera2d_state.pan_offset.y;

    Vec2::new(world_x, world_y)
}

/// Convert 2D world coordinates to screen coordinates
fn world_to_screen_2d(world_pos: Vec2, viewport: &ViewportState, camera2d_state: &Camera2DState) -> Vec2 {
    let viewport_center = Vec2::new(
        viewport.position[0] + viewport.size[0] / 2.0,
        viewport.position[1] + viewport.size[1] / 2.0,
    );

    let relative_world = world_pos - camera2d_state.pan_offset;
    let screen_x = relative_world.x * camera2d_state.zoom + viewport_center.x;
    let screen_y = -relative_world.y * camera2d_state.zoom + viewport_center.y;

    Vec2::new(screen_x, screen_y)
}

/// Calculate distance from a point to a line segment
fn distance_to_line_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let line = end - start;
    let len_sq = line.length_squared();

    if len_sq == 0.0 {
        return (point - start).length();
    }

    let t = ((point - start).dot(line) / len_sq).clamp(0.0, 1.0);
    let projection = start + line * t;

    (point - projection).length()
}
