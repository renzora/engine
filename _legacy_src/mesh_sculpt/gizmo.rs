//! Brush preview gizmo for mesh sculpting

use bevy::prelude::*;

use crate::core::ViewportState;
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{TerrainBrushType, TerrainSettings};

use super::data::MeshSculptState;

/// System to draw a brush circle on the mesh surface at the hover point.
pub fn mesh_sculpt_gizmo_system(
    gizmo_state: Res<GizmoState>,
    sculpt_state: Res<MeshSculptState>,
    settings: Res<TerrainSettings>,
    viewport: Res<ViewportState>,
    mut gizmos: Gizmos,
) {
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        return;
    }

    if !viewport.hovered || !sculpt_state.brush_visible {
        return;
    }

    let Some(hover_pos) = sculpt_state.hover_position else {
        return;
    };
    let hover_normal = sculpt_state.hover_normal.unwrap_or(Vec3::Y);

    let color = match settings.brush_type {
        TerrainBrushType::Raise => Color::srgba(0.2, 0.8, 0.2, 0.8),
        TerrainBrushType::Lower => Color::srgba(0.8, 0.4, 0.2, 0.8),
        TerrainBrushType::Sculpt => Color::srgba(0.2, 0.8, 0.2, 0.8),
        TerrainBrushType::Erase => Color::srgba(0.8, 0.2, 0.2, 0.8),
        TerrainBrushType::Smooth => Color::srgba(0.2, 0.5, 0.8, 0.8),
        TerrainBrushType::Flatten => Color::srgba(0.8, 0.8, 0.2, 0.8),
        _ => Color::srgba(0.6, 0.6, 0.6, 0.8),
    };

    let brush_radius = settings.brush_radius;

    // Build a tangent frame from the surface normal
    let (tangent, bitangent) = build_tangent_frame(hover_normal);

    // Draw outer circle in the tangent plane
    let segments = 48;
    draw_circle_in_plane(
        &mut gizmos,
        hover_pos,
        &tangent,
        &bitangent,
        hover_normal,
        brush_radius,
        segments,
        color,
    );

    // Draw inner falloff circle when falloff < 1.0
    if settings.falloff < 0.99 {
        let inner_radius = brush_radius * (1.0 - settings.falloff);
        let inner_color = color.with_alpha(0.4);
        draw_circle_in_plane(
            &mut gizmos,
            hover_pos,
            &tangent,
            &bitangent,
            hover_normal,
            inner_radius,
            segments,
            inner_color,
        );
    }
}

/// Build an orthonormal tangent frame from a normal vector.
fn build_tangent_frame(normal: Vec3) -> (Vec3, Vec3) {
    // Pick a reference vector that's not parallel to the normal
    let reference = if normal.y.abs() > 0.99 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let tangent = normal.cross(reference).normalize();
    let bitangent = normal.cross(tangent).normalize();
    (tangent, bitangent)
}

/// Draw a circle as line segments in a tangent plane, slightly offset along the normal.
fn draw_circle_in_plane(
    gizmos: &mut Gizmos,
    center: Vec3,
    tangent: &Vec3,
    bitangent: &Vec3,
    normal: Vec3,
    radius: f32,
    segments: usize,
    color: Color,
) {
    let offset = normal * 0.02; // Slight offset to avoid z-fighting
    let mut points: Vec<Vec3> = Vec::with_capacity(segments);

    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin_a, cos_a) = angle.sin_cos();
        let point = center + *tangent * (cos_a * radius) + *bitangent * (sin_a * radius) + offset;
        points.push(point);
    }

    for i in 0..segments {
        let next = (i + 1) % segments;
        gizmos.line(points[i], points[next], color);
    }
}
