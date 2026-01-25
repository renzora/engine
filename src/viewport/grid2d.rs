//! 2D grid rendering for the editor viewport
//!
//! Draws an adaptive grid in 2D mode that scales with zoom level.

use bevy::prelude::*;

use crate::core::{EditorSettings, ViewportMode, ViewportState};
use crate::gizmo::GridGizmoGroup;
use super::Camera2DState;

/// Draw an adaptive 2D grid that scales with zoom level
pub fn draw_grid_2d(
    mut gizmos: Gizmos<GridGizmoGroup>,
    settings: Res<EditorSettings>,
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
) {
    // Only draw in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    if !settings.show_grid {
        return;
    }

    // Calculate adaptive grid spacing based on zoom
    // We want the grid to always look reasonable regardless of zoom
    let base_spacing = 100.0; // Base spacing in pixels at 100% zoom
    let zoom = camera2d_state.zoom;

    // Calculate the actual world-space spacing
    // As we zoom in, we want smaller grid lines to appear
    // As we zoom out, we want larger grid lines (combine smaller ones)
    let world_spacing = calculate_adaptive_spacing(base_spacing, zoom);

    // Calculate visible area based on viewport size and zoom
    let half_width = (viewport.size[0] / zoom / 2.0) * 1.5; // Extra margin
    let half_height = (viewport.size[1] / zoom / 2.0) * 1.5;

    let center = camera2d_state.pan_offset;

    // Snap grid to spacing to prevent "swimming"
    let start_x = ((center.x - half_width) / world_spacing).floor() * world_spacing;
    let end_x = ((center.x + half_width) / world_spacing).ceil() * world_spacing;
    let start_y = ((center.y - half_height) / world_spacing).floor() * world_spacing;
    let end_y = ((center.y + half_height) / world_spacing).ceil() * world_spacing;

    let gc = settings.grid_color;
    let grid_color = Color::srgba(gc[0], gc[1], gc[2], 0.25);
    let grid_color_major = Color::srgba(gc[0], gc[1], gc[2], 0.5);

    // Axis colors
    let axis_color_x = Color::srgba(0.9, 0.2, 0.2, 0.9); // Red for X
    let axis_color_y = Color::srgba(0.2, 0.9, 0.2, 0.9); // Green for Y

    // Draw vertical lines (varying X)
    let mut x = start_x;
    while x <= end_x {
        let is_origin = x.abs() < world_spacing * 0.01;
        let is_major = !is_origin && ((x / world_spacing).round() as i32 % 10 == 0);

        let color = if is_origin {
            axis_color_y // Y axis
        } else if is_major {
            grid_color_major
        } else {
            grid_color
        };

        gizmos.line(
            Vec3::new(x, start_y, 0.0),
            Vec3::new(x, end_y, 0.0),
            color,
        );
        x += world_spacing;
    }

    // Draw horizontal lines (varying Y)
    let mut y = start_y;
    while y <= end_y {
        let is_origin = y.abs() < world_spacing * 0.01;
        let is_major = !is_origin && ((y / world_spacing).round() as i32 % 10 == 0);

        let color = if is_origin {
            axis_color_x // X axis
        } else if is_major {
            grid_color_major
        } else {
            grid_color
        };

        gizmos.line(
            Vec3::new(start_x, y, 0.0),
            Vec3::new(end_x, y, 0.0),
            color,
        );
        y += world_spacing;
    }
}

/// Calculate adaptive grid spacing based on zoom level
/// Returns a "nice" round number that scales well with zoom
fn calculate_adaptive_spacing(base_spacing: f32, zoom: f32) -> f32 {
    // Target spacing in screen pixels
    let target_screen_spacing = base_spacing;

    // World-space spacing that would give us target_screen_spacing at current zoom
    let ideal_world_spacing = target_screen_spacing / zoom;

    // Round to a "nice" number (power of 10, or 2, 5 times power of 10)
    let nice_numbers = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0];

    // Find the closest nice number
    let mut best_spacing = nice_numbers[0];
    let mut best_diff = (ideal_world_spacing - best_spacing).abs();

    for &nice in &nice_numbers {
        let diff = (ideal_world_spacing - nice).abs();
        if diff < best_diff {
            best_diff = diff;
            best_spacing = nice;
        }
    }

    best_spacing
}
