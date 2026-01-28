//! Debug draw rendering system
//!
//! Renders debug shapes from scripts using Bevy gizmos.

use bevy::prelude::*;
use crate::scripting::resources::{DebugDrawCommand, DebugDrawQueue};
use crate::core::PlayModeState;

/// System to tick debug draw durations
pub fn tick_debug_draws(
    time: Res<Time>,
    play_mode: Res<PlayModeState>,
    mut draws: ResMut<DebugDrawQueue>,
) {
    // Only tick during play mode
    if !play_mode.is_playing() {
        return;
    }

    draws.tick(time.delta_secs());
}

/// System to render debug draws using gizmos
pub fn render_debug_draws(
    draws: Res<DebugDrawQueue>,
    play_mode: Res<PlayModeState>,
    mut gizmos: Gizmos,
) {
    // Only render during play mode
    if !play_mode.is_in_play_mode() {
        return;
    }

    for draw in draws.get_draws() {
        match draw {
            DebugDrawCommand::Line { start, end, color, .. } => {
                gizmos.line(*start, *end, *color);
            }

            DebugDrawCommand::Ray { origin, direction, length, color, .. } => {
                let end = *origin + direction.normalize() * *length;
                gizmos.line(*origin, end, *color);
                // Draw arrowhead
                draw_arrowhead(&mut gizmos, end, *direction, *color, *length * 0.1);
            }

            DebugDrawCommand::Sphere { center, radius, color, .. } => {
                // Draw a sphere using three circles for each axis
                gizmos.sphere(Isometry3d::from_translation(*center), *radius, *color);
            }

            DebugDrawCommand::Box { center, half_extents, color, .. } => {
                // Draw a wireframe box using lines
                let min = *center - *half_extents;
                let max = *center + *half_extents;

                // Bottom face
                gizmos.line(Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z), *color);
                gizmos.line(Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, min.y, max.z), *color);
                gizmos.line(Vec3::new(max.x, min.y, max.z), Vec3::new(min.x, min.y, max.z), *color);
                gizmos.line(Vec3::new(min.x, min.y, max.z), Vec3::new(min.x, min.y, min.z), *color);

                // Top face
                gizmos.line(Vec3::new(min.x, max.y, min.z), Vec3::new(max.x, max.y, min.z), *color);
                gizmos.line(Vec3::new(max.x, max.y, min.z), Vec3::new(max.x, max.y, max.z), *color);
                gizmos.line(Vec3::new(max.x, max.y, max.z), Vec3::new(min.x, max.y, max.z), *color);
                gizmos.line(Vec3::new(min.x, max.y, max.z), Vec3::new(min.x, max.y, min.z), *color);

                // Vertical edges
                gizmos.line(Vec3::new(min.x, min.y, min.z), Vec3::new(min.x, max.y, min.z), *color);
                gizmos.line(Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, max.y, min.z), *color);
                gizmos.line(Vec3::new(max.x, min.y, max.z), Vec3::new(max.x, max.y, max.z), *color);
                gizmos.line(Vec3::new(min.x, min.y, max.z), Vec3::new(min.x, max.y, max.z), *color);
            }

            DebugDrawCommand::Point { position, size, color, .. } => {
                // Draw a small cross at the point
                let s = *size * 0.5;
                gizmos.line(*position - Vec3::X * s, *position + Vec3::X * s, *color);
                gizmos.line(*position - Vec3::Y * s, *position + Vec3::Y * s, *color);
                gizmos.line(*position - Vec3::Z * s, *position + Vec3::Z * s, *color);
            }
        }
    }
}

/// Draw an arrowhead at the end of a ray
fn draw_arrowhead(gizmos: &mut Gizmos, tip: Vec3, direction: Vec3, color: Color, size: f32) {
    let dir = direction.normalize();

    // Find perpendicular vectors
    let perp1 = if dir.abs().dot(Vec3::Y) < 0.999 {
        dir.cross(Vec3::Y).normalize()
    } else {
        dir.cross(Vec3::X).normalize()
    };
    let perp2 = dir.cross(perp1).normalize();

    // Draw four lines back from the tip
    let back = tip - dir * size;
    gizmos.line(tip, back + perp1 * size * 0.3, color);
    gizmos.line(tip, back - perp1 * size * 0.3, color);
    gizmos.line(tip, back + perp2 * size * 0.3, color);
    gizmos.line(tip, back - perp2 * size * 0.3, color);
}

/// System to clear debug draws when exiting play mode
pub fn clear_debug_draws_on_stop(
    play_mode: Res<PlayModeState>,
    mut draws: ResMut<DebugDrawQueue>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_playing() || play_mode.is_paused();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        draws.clear();
    }

    *last_playing = currently_playing;
}
