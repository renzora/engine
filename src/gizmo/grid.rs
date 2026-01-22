use bevy::prelude::*;
use crate::core::EditorSettings;

/// Draw a reference grid on the XZ plane
pub fn draw_grid(mut gizmos: Gizmos, settings: Res<EditorSettings>) {
    if !settings.show_grid {
        return;
    }

    let grid_size = settings.grid_divisions;
    let grid_spacing = settings.grid_size / grid_size as f32;
    let half_size = settings.grid_size / 2.0;

    let gc = settings.grid_color;
    let grid_color = Color::srgba(gc[0], gc[1], gc[2], 0.5);
    let axis_color_x = Color::srgba(0.8, 0.3, 0.3, 0.7);
    let axis_color_z = Color::srgba(0.3, 0.3, 0.8, 0.7);

    // Draw grid lines parallel to X axis
    for i in 0..=grid_size {
        let z = -half_size + i as f32 * grid_spacing;
        let color = if i == grid_size / 2 { axis_color_x } else { grid_color };
        gizmos.line(
            Vec3::new(-half_size, 0.0, z),
            Vec3::new(half_size, 0.0, z),
            color,
        );
    }

    // Draw grid lines parallel to Z axis
    for i in 0..=grid_size {
        let x = -half_size + i as f32 * grid_spacing;
        let color = if i == grid_size / 2 { axis_color_z } else { grid_color };
        gizmos.line(
            Vec3::new(x, 0.0, -half_size),
            Vec3::new(x, 0.0, half_size),
            color,
        );
    }
}
