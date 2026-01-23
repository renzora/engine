use bevy::prelude::*;
use crate::core::{EditorSettings, OrbitCameraState};
use super::GridGizmoGroup;

/// Draw an infinite reference grid on the XZ plane that follows the camera
pub fn draw_grid(mut gizmos: Gizmos<GridGizmoGroup>, settings: Res<EditorSettings>, camera: Res<OrbitCameraState>) {
    if !settings.show_grid {
        return;
    }

    // Grid spacing (distance between lines)
    let grid_spacing = settings.grid_size / settings.grid_divisions as f32;

    // Calculate grid extent based on camera distance (make it feel infinite)
    let grid_extent = (camera.distance * 2.0).max(50.0);
    let half_extent = grid_extent / 2.0;

    // Snap grid center to grid spacing to avoid grid "swimming" as camera moves
    let center_x = (camera.focus.x / grid_spacing).floor() * grid_spacing;
    let center_z = (camera.focus.z / grid_spacing).floor() * grid_spacing;

    // Calculate number of lines needed
    let line_count = ((grid_extent / grid_spacing) as i32).max(20);

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
        let is_major = (i % 10) == 0;
        let is_origin = z.abs() < grid_spacing * 0.5;

        let color = if is_origin {
            axis_color_x
        } else if is_major {
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
        let is_major = (i % 10) == 0;
        let is_origin = x.abs() < grid_spacing * 0.5;

        let color = if is_origin {
            axis_color_z
        } else if is_major {
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

    // Draw Y axis (green, vertical line at origin)
    let axis_length = grid_extent * 0.5;
    gizmos.line(
        Vec3::new(0.0, -axis_length, 0.0),
        Vec3::new(0.0, axis_length, 0.0),
        axis_color_y,
    );
}
