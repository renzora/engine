use bevy::prelude::*;
use crate::core::{EditorSettings, OrbitCameraState, PlayModeState, PlayState};
use super::{GridGizmoGroup, AxisGizmoGroup};

/// Snap a number to a "nice" value (1, 2, 5, 10, 20, 50, 100, etc.)
fn snap_to_nice_number(n: i32) -> i32 {
    if n <= 1 { return 1; }
    if n <= 2 { return 2; }
    if n <= 5 { return 5; }

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
pub fn draw_grid(
    mut gizmos: Gizmos<GridGizmoGroup>,
    mut axis_gizmos: Gizmos<AxisGizmoGroup>,
    settings: Res<EditorSettings>,
    camera: Res<OrbitCameraState>,
    play_mode: Res<PlayModeState>,
) {
    if !settings.show_grid || matches!(play_mode.state, PlayState::Playing | PlayState::Paused) {
        return;
    }

    let base_spacing = settings.grid_size / settings.grid_divisions as f32;

    let max_lines = 150;
    let min_extent = camera.distance * 5.0;
    let needed_spacing = min_extent / max_lines as f32;
    let spacing_multiplier = (needed_spacing / base_spacing).max(1.0).ceil() as i32;
    let nice_multiplier = snap_to_nice_number(spacing_multiplier);
    let grid_spacing = base_spacing * nice_multiplier as f32;

    let grid_extent = grid_spacing * max_lines as f32;
    let half_extent = grid_extent / 2.0;

    let axis_extent = camera.distance * 100.0;

    let center_x = (camera.focus.x / grid_spacing).floor() * grid_spacing;
    let center_z = (camera.focus.z / grid_spacing).floor() * grid_spacing;

    let line_count = (half_extent / grid_spacing) as i32;

    let gc = settings.grid_color;
    let grid_color = Color::srgba(gc[0], gc[1], gc[2], 0.3);
    let grid_color_major = Color::srgba(gc[0], gc[1], gc[2], 0.5);
    let grid_color_sub = Color::srgba(gc[0], gc[1], gc[2], 0.15);

    let axis_color_x = Color::srgba(0.9, 0.2, 0.2, 0.4);
    let axis_color_y = Color::srgba(0.2, 0.9, 0.2, 0.4);
    let axis_color_z = Color::srgba(0.2, 0.2, 0.9, 0.4);

    // ── Sub-grid ──────────────────────────────────────────────────────────────
    if settings.show_subgrid {
        let sub_spacing = grid_spacing / 10.0;
        let sub_cx = (camera.focus.x / sub_spacing).floor() * sub_spacing;
        let sub_cz = (camera.focus.z / sub_spacing).floor() * sub_spacing;
        let sub_line_count = (half_extent / sub_spacing) as i32;

        for i in -sub_line_count..=sub_line_count {
            let z = sub_cz + i as f32 * sub_spacing;
            // Skip positions that coincide with a main grid line or the origin
            if ((z / grid_spacing).round() * grid_spacing - z).abs() < sub_spacing * 0.1 { continue; }
            if z.abs() < sub_spacing * 0.5 { continue; }
            gizmos.line(
                Vec3::new(sub_cx - half_extent, 0.0, z),
                Vec3::new(sub_cx + half_extent, 0.0, z),
                grid_color_sub,
            );
        }

        for i in -sub_line_count..=sub_line_count {
            let x = sub_cx + i as f32 * sub_spacing;
            if ((x / grid_spacing).round() * grid_spacing - x).abs() < sub_spacing * 0.1 { continue; }
            if x.abs() < sub_spacing * 0.5 { continue; }
            gizmos.line(
                Vec3::new(x, 0.0, sub_cz - half_extent),
                Vec3::new(x, 0.0, sub_cz + half_extent),
                grid_color_sub,
            );
        }
    }

    // ── Main grid ─────────────────────────────────────────────────────────────
    for i in -line_count..=line_count {
        let z = center_z + i as f32 * grid_spacing;
        if z.abs() < grid_spacing * 0.5 { continue; }

        let world_index = (z / base_spacing).round() as i32;
        let color = if world_index % 10 == 0 { grid_color_major } else { grid_color };

        gizmos.line(
            Vec3::new(center_x - half_extent, 0.0, z),
            Vec3::new(center_x + half_extent, 0.0, z),
            color,
        );
    }

    for i in -line_count..=line_count {
        let x = center_x + i as f32 * grid_spacing;
        if x.abs() < grid_spacing * 0.5 { continue; }

        let world_index = (x / base_spacing).round() as i32;
        let color = if world_index % 10 == 0 { grid_color_major } else { grid_color };

        gizmos.line(
            Vec3::new(x, 0.0, center_z - half_extent),
            Vec3::new(x, 0.0, center_z + half_extent),
            color,
        );
    }

    // ── Axis lines — drawn with AxisGizmoGroup so they render in front of grid ─
    axis_gizmos.line(
        Vec3::new(-axis_extent, 0.0, 0.0),
        Vec3::new(axis_extent, 0.0, 0.0),
        axis_color_x,
    );
    axis_gizmos.line(
        Vec3::new(0.0, -axis_extent, 0.0),
        Vec3::new(0.0, axis_extent, 0.0),
        axis_color_y,
    );
    axis_gizmos.line(
        Vec3::new(0.0, 0.0, -axis_extent),
        Vec3::new(0.0, 0.0, axis_extent),
        axis_color_z,
    );
}
