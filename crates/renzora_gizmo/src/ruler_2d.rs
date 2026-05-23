//! 2D viewport ruler — top + left strips showing world-coordinate ticks.
//!
//! Adapts tick spacing to the camera's current zoom so a useful number of
//! major ticks (~6–10) are visible at any scale. Painted as a
//! `ViewportOverlayRegistry` callback at a low order so it sits *under*
//! gizmos and selection outlines.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};

/// Width (vertical ruler) / height (horizontal ruler) of the strip in
/// panel pixels.
const RULER_THICKNESS: f32 = 18.0;
const TICK_LONG: f32 = 8.0;
const TICK_SHORT: f32 = 4.0;
const SUBDIVISIONS: u32 = 5;

fn find_editor_camera_2d(world: &World) -> Option<(&Camera, &GlobalTransform)> {
    let entity = world
        .get_resource::<crate::light_gizmo::SceneIconCache>()?
        .editor_camera_2d?;
    let camera = world.get::<Camera>(entity)?;
    let cam_gt = world.get::<GlobalTransform>(entity)?;
    Some((camera, cam_gt))
}

/// Pick a "nice" tick spacing in world units given the visible world
/// extent — aiming for roughly `target_count` major ticks across the
/// span. Snaps to 1, 2, or 5 × 10ⁿ so labels read cleanly.
fn nice_tick_spacing(world_span: f32, target_count: f32) -> f32 {
    if world_span <= 0.0 || target_count <= 0.0 {
        return 1.0;
    }
    let raw = (world_span / target_count).max(f32::EPSILON);
    let pow = 10f32.powf(raw.log10().floor());
    let normalized = raw / pow;
    let nice = if normalized < 2.0 {
        1.0
    } else if normalized < 5.0 {
        2.0
    } else {
        5.0
    };
    nice * pow
}

/// Egui overlay drawer.
pub fn draw_ruler_2d(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    if view != ViewportView::Two {
        return;
    }
    let Some(viewport) = world.get_resource::<ViewportState>() else {
        return;
    };
    let Some((camera, cam_gt)) = find_editor_camera_2d(world) else {
        return;
    };

    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    // Helpers: panel-pixel ↔ image-pixel ↔ world.
    let panel_to_image = |p: Vec2| -> Vec2 {
        Vec2::new(
            p.x * image_size.x / rect.width(),
            p.y * image_size.y / rect.height(),
        )
    };
    let image_to_panel = |p: Vec2| -> Vec2 {
        Vec2::new(
            p.x * rect.width() / image_size.x,
            p.y * rect.height() / image_size.y,
        )
    };
    let panel_to_world = |panel: Vec2| -> Option<Vec2> {
        camera
            .viewport_to_world_2d(cam_gt, panel_to_image(panel))
            .ok()
    };
    let world_to_panel = |w: Vec3| -> Option<Vec2> {
        let img = camera.world_to_viewport(cam_gt, w).ok()?;
        Some(image_to_panel(img))
    };

    // Visible world AABB at the viewport corners.
    let (Some(tl_world), Some(br_world)) = (
        panel_to_world(Vec2::ZERO),
        panel_to_world(Vec2::new(rect.width(), rect.height())),
    ) else {
        return;
    };

    let world_min_x = tl_world.x.min(br_world.x);
    let world_max_x = tl_world.x.max(br_world.x);
    // Screen y is down, world y is up, so the panel-top corner has the
    // *higher* world y and the bottom-corner has the lower.
    let world_max_y = tl_world.y.max(br_world.y);
    let world_min_y = tl_world.y.min(br_world.y);

    let world_w = world_max_x - world_min_x;
    let world_h = world_max_y - world_min_y;
    let tick_x = nice_tick_spacing(world_w, 8.0);
    let tick_y = nice_tick_spacing(world_h, 6.0);

    let painter = ui.painter_at(rect);
    let bg = egui::Color32::from_rgba_unmultiplied(15, 17, 22, 220);
    let major_color = egui::Color32::from_white_alpha(190);
    let minor_color = egui::Color32::from_white_alpha(80);
    let label_color = egui::Color32::from_white_alpha(170);
    let origin_color = egui::Color32::from_rgb(255, 200, 80);
    let font = egui::FontId::monospace(9.0);

    // ── Horizontal ruler (top strip) ────────────────────────────────────
    let h_rect =
        egui::Rect::from_min_size(rect.min, egui::Vec2::new(rect.width(), RULER_THICKNESS));
    painter.rect_filled(h_rect, 0.0, bg);

    let minor_x = tick_x / SUBDIVISIONS as f32;
    let mut x = (world_min_x / minor_x).floor() * minor_x;
    while x <= world_max_x {
        let Some(panel) = world_to_panel(Vec3::new(x, 0.0, 0.0)) else {
            x += minor_x;
            continue;
        };
        let screen_x = rect.min.x + panel.x;
        let is_major = ((x / tick_x).round() * tick_x - x).abs() < (minor_x * 0.01);
        let is_origin = x.abs() < (minor_x * 0.01);
        let tick_len = if is_major { TICK_LONG } else { TICK_SHORT };
        let stroke_color = if is_origin {
            origin_color
        } else if is_major {
            major_color
        } else {
            minor_color
        };
        painter.line_segment(
            [
                egui::Pos2::new(screen_x, h_rect.max.y - tick_len),
                egui::Pos2::new(screen_x, h_rect.max.y),
            ],
            egui::Stroke::new(1.0, stroke_color),
        );
        if is_major {
            painter.text(
                egui::Pos2::new(screen_x + 2.0, h_rect.min.y + 1.0),
                egui::Align2::LEFT_TOP,
                format_tick(x, tick_x),
                font.clone(),
                if is_origin { origin_color } else { label_color },
            );
        }
        x += minor_x;
    }

    // ── Vertical ruler (left strip) ─────────────────────────────────────
    let v_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.min.x, rect.min.y + RULER_THICKNESS),
        egui::Vec2::new(RULER_THICKNESS, rect.height() - RULER_THICKNESS),
    );
    painter.rect_filled(v_rect, 0.0, bg);

    let minor_y = tick_y / SUBDIVISIONS as f32;
    let mut y = (world_min_y / minor_y).floor() * minor_y;
    while y <= world_max_y {
        let Some(panel) = world_to_panel(Vec3::new(0.0, y, 0.0)) else {
            y += minor_y;
            continue;
        };
        let screen_y = rect.min.y + panel.y;
        if screen_y < v_rect.min.y || screen_y > v_rect.max.y {
            y += minor_y;
            continue;
        }
        let is_major = ((y / tick_y).round() * tick_y - y).abs() < (minor_y * 0.01);
        let is_origin = y.abs() < (minor_y * 0.01);
        let tick_len = if is_major { TICK_LONG } else { TICK_SHORT };
        let stroke_color = if is_origin {
            origin_color
        } else if is_major {
            major_color
        } else {
            minor_color
        };
        painter.line_segment(
            [
                egui::Pos2::new(v_rect.max.x - tick_len, screen_y),
                egui::Pos2::new(v_rect.max.x, screen_y),
            ],
            egui::Stroke::new(1.0, stroke_color),
        );
        if is_major {
            // Render label sideways-ish — center on the tick, small font,
            // truncated to fit the 18-pixel strip.
            painter.text(
                egui::Pos2::new(v_rect.min.x + 2.0, screen_y - 1.0),
                egui::Align2::LEFT_BOTTOM,
                format_tick(y, tick_y),
                font.clone(),
                if is_origin { origin_color } else { label_color },
            );
        }
        y += minor_y;
    }

    // Corner square so the two strips meet cleanly.
    let corner =
        egui::Rect::from_min_size(rect.min, egui::Vec2::new(RULER_THICKNESS, RULER_THICKNESS));
    painter.rect_filled(corner, 0.0, bg);
}

/// Format a world-coordinate label tightly. Drops trailing zeroes when
/// the tick spacing is an integer; uses a fixed precision when ticks are
/// fractional.
fn format_tick(value: f32, tick: f32) -> String {
    if tick >= 1.0 {
        format!("{:.0}", value)
    } else if tick >= 0.1 {
        format!("{:.1}", value)
    } else if tick >= 0.01 {
        format!("{:.2}", value)
    } else {
        format!("{:.3}", value)
    }
}
