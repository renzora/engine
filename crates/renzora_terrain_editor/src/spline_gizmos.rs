//! Gizmo overlay for [`SplinePath`] entities.
//!
//! Draws the control-point polyline and a smooth Catmull-Rom curve for every
//! spline in the scene, plus small markers at each control point. Selected
//! splines are highlighted.

use bevy::color::palettes::css;
use bevy::prelude::*;

use renzora_spline::SplinePath;

/// Samples per spline segment when drawing the smooth curve. 24 is smooth
/// enough for most real spline lengths without flooding the gizmo buffer.
const SAMPLES_PER_SEGMENT: usize = 24;

/// Radius of the sphere drawn at each control point, in world units.
const HANDLE_RADIUS: f32 = 0.35;

pub fn draw_spline_gizmos_system(
    mut gizmos: Gizmos,
    splines: Query<(Entity, &GlobalTransform, &SplinePath)>,
    selection: Option<Res<renzora_editor_framework::EditorSelection>>,
) {
    let selected_entity = selection.as_ref().and_then(|s| s.get());

    for (entity, gt, path) in splines.iter() {
        let is_selected = Some(entity) == selected_entity;
        let curve_color: Color = if is_selected {
            css::YELLOW.into()
        } else {
            css::ORANGE.into()
        };
        let handle_color: Color = if is_selected {
            css::WHITE.into()
        } else {
            css::LIGHT_GRAY.into()
        };

        let to_world = |p: Vec3| gt.transform_point(p);

        // Smooth curve: sample the spline and draw as a line strip.
        let segments = path.segment_count();
        if segments >= 1 && path.control_points.len() >= 2 {
            let sample_count = segments * SAMPLES_PER_SEGMENT + 1;
            let mut prev = to_world(path.sample(0.0));
            for i in 1..sample_count {
                let t = i as f32 / SAMPLES_PER_SEGMENT as f32;
                let next = to_world(path.sample(t));
                gizmos.line(prev, next, curve_color);
                prev = next;
            }
        }

        // Handle markers + straight polyline between control points.
        for (i, p) in path.control_points.iter().enumerate() {
            let wp = to_world(*p);
            gizmos.sphere(wp, HANDLE_RADIUS, handle_color);
            if i + 1 < path.control_points.len() {
                let wn = to_world(path.control_points[i + 1]);
                // Faint chord line so the user can still see relation between
                // points when the curve is tightly curved.
                gizmos.line(wp, wn, handle_color.with_alpha(0.25));
            }
        }
    }
}
