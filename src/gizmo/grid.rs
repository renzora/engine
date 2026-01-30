use bevy::prelude::*;
use crate::core::{EditorSettings, OrbitCameraState};
use super::GridGizmoGroup;

/// Snap a number to a "nice" value (1, 2, 5, 10, 20, 50, 100, etc.)
fn snap_to_nice_number(n: i32) -> i32 {
    if n <= 1 { return 1; }
    if n <= 2 { return 2; }
    if n <= 5 { return 5; }

    // Find the order of magnitude
    let magnitude = 10_i32.pow((n as f32).log10().floor() as u32);
    let normalized = n / magnitude;

    let nice = if normalized <= 1 {
        1
    } else if normalized <= 2 {
        2
    } else if normalized <= 5 {
        5
    } else {
        10
    };

    nice * magnitude
}

/// Draw an infinite reference grid on the XZ plane that follows the camera
pub fn draw_grid(mut gizmos: Gizmos<GridGizmoGroup>, settings: Res<EditorSettings>, camera: Res<OrbitCameraState>) {
    if !settings.show_grid {
        return;
    }

    // Base grid spacing from settings
    let base_spacing = settings.grid_size / settings.grid_divisions as f32;

    // Adaptive grid: increase spacing when zoomed out to limit line count
    // Target around 100-200 lines max for performance
    let max_lines = 150;
    let min_extent = camera.distance * 5.0;

    // Calculate what spacing we need to stay under max_lines
    let needed_spacing = min_extent / max_lines as f32;

    // Round up to next power of base_spacing (so grid stays aligned)
    let spacing_multiplier = (needed_spacing / base_spacing).max(1.0).ceil() as i32;
    // Snap to nice multipliers: 1, 2, 5, 10, 20, 50, 100, etc.
    let nice_multiplier = snap_to_nice_number(spacing_multiplier);
    let grid_spacing = base_spacing * nice_multiplier as f32;

    // Grid extent - make it large enough to cover the view
    let grid_extent = grid_spacing * max_lines as f32;
    let half_extent = grid_extent / 2.0;

    // Axis lines should extend much further than the grid (appear infinite)
    let axis_extent = camera.distance * 100.0;

    // Snap grid center to grid spacing to avoid grid "swimming" as camera moves
    let center_x = (camera.focus.x / grid_spacing).floor() * grid_spacing;
    let center_z = (camera.focus.z / grid_spacing).floor() * grid_spacing;

    // Calculate number of lines needed
    let line_count = (half_extent / grid_spacing) as i32;

    let gc = settings.grid_color;
    let grid_color = Color::srgba(gc[0], gc[1], gc[2], 0.3);
    let grid_color_major = Color::srgba(gc[0], gc[1], gc[2], 0.5);

    // Axis colors (X=Red, Y=Green, Z=Blue)
    let axis_color_x = Color::srgba(0.9, 0.2, 0.2, 0.9);
    let axis_color_y = Color::srgba(0.2, 0.9, 0.2, 0.9);
    let axis_color_z = Color::srgba(0.2, 0.2, 0.9, 0.9);

    // Draw grid lines parallel to X axis (varying Z)
    for i in -line_count..=line_count {
        let z = center_z + i as f32 * grid_spacing;
        let is_origin = z.abs() < grid_spacing * 0.5;

        // Skip origin lines - they're drawn separately as infinite axes
        if is_origin {
            continue;
        }

        // Major lines every 10 base spacings
        let world_index = (z / base_spacing).round() as i32;
        let is_major = world_index % 10 == 0;

        let color = if is_major {
            grid_color_major
        } else {
            grid_color
        };

        gizmos.line(
            Vec3::new(center_x - half_extent, 0.0, z),
            Vec3::new(center_x + half_extent, 0.0, z),
            color,
        );
    }

    // Draw grid lines parallel to Z axis (varying X)
    for i in -line_count..=line_count {
        let x = center_x + i as f32 * grid_spacing;
        let is_origin = x.abs() < grid_spacing * 0.5;

        // Skip origin lines - they're drawn separately as infinite axes
        if is_origin {
            continue;
        }

        // Major lines every 10 base spacings
        let world_index = (x / base_spacing).round() as i32;
        let is_major = world_index % 10 == 0;

        let color = if is_major {
            grid_color_major
        } else {
            grid_color
        };

        gizmos.line(
            Vec3::new(x, 0.0, center_z - half_extent),
            Vec3::new(x, 0.0, center_z + half_extent),
            color,
        );
    }

    // Draw infinite axis lines (extend far beyond grid)
    // X axis (red)
    gizmos.line(
        Vec3::new(-axis_extent, 0.0, 0.0),
        Vec3::new(axis_extent, 0.0, 0.0),
        axis_color_x,
    );
    // Y axis (green, vertical)
    gizmos.line(
        Vec3::new(0.0, -axis_extent, 0.0),
        Vec3::new(0.0, axis_extent, 0.0),
        axis_color_y,
    );
    // Z axis (blue)
    gizmos.line(
        Vec3::new(0.0, 0.0, -axis_extent),
        Vec3::new(0.0, 0.0, axis_extent),
        axis_color_z,
    );
}
