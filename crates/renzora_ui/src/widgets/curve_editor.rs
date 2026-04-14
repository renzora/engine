//! Curve editor — piecewise bezier curve with tangent handles.
//!
//! Designed for animation curves, audio envelopes, particle lifetime
//! properties, post-process ramps. Keys are `(t, value)` pairs with optional
//! `(in_tangent, out_tangent)` offsets. The widget mutates the keys in place.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// A single curve key with bezier tangent offsets. Linear keys set both
/// tangents to zero.
#[derive(Clone, Copy, Debug)]
pub struct CurveKey {
    pub t: f32,
    pub value: f32,
    pub in_tangent: Vec2,
    pub out_tangent: Vec2,
}

impl CurveKey {
    pub fn linear(t: f32, value: f32) -> Self {
        Self { t, value, in_tangent: Vec2::ZERO, out_tangent: Vec2::ZERO }
    }
}

/// Configuration for `curve_editor`.
#[derive(Clone, Copy, Debug)]
pub struct CurveEditorConfig {
    pub height: f32,
    pub t_range: (f32, f32),
    pub value_range: (f32, f32),
    pub show_grid: bool,
}

impl Default for CurveEditorConfig {
    fn default() -> Self {
        Self {
            height: 160.0,
            t_range: (0.0, 1.0),
            value_range: (0.0, 1.0),
            show_grid: true,
        }
    }
}

/// Interactive curve editor. Click empty space to add a key, drag keys to
/// move them, double-click a key to delete it.
pub fn curve_editor(
    ui: &mut egui::Ui,
    keys: &mut Vec<CurveKey>,
    cfg: CurveEditorConfig,
    theme: &Theme,
) -> egui::Response {
    let w = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, cfg.height), Sense::click_and_drag());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme.surfaces.faint.to_color32());
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    if cfg.show_grid {
        let grid_color = theme.widgets.border.to_color32().gamma_multiply(0.4);
        for i in 1..4 {
            let f = i as f32 / 4.0;
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + rect.width() * f, rect.min.y),
                    Pos2::new(rect.min.x + rect.width() * f, rect.max.y),
                ],
                Stroke::new(0.5, grid_color),
            );
            painter.line_segment(
                [
                    Pos2::new(rect.min.x, rect.min.y + rect.height() * f),
                    Pos2::new(rect.max.x, rect.min.y + rect.height() * f),
                ],
                Stroke::new(0.5, grid_color),
            );
        }
    }

    let (tmin, tmax) = cfg.t_range;
    let (vmin, vmax) = cfg.value_range;
    let to_screen = |k: &CurveKey| -> Pos2 {
        Pos2::new(
            rect.min.x + rect.width() * ((k.t - tmin) / (tmax - tmin)),
            rect.max.y - rect.height() * ((k.value - vmin) / (vmax - vmin)),
        )
    };
    let to_curve = |p: Pos2| -> (f32, f32) {
        (
            tmin + (p.x - rect.min.x) / rect.width() * (tmax - tmin),
            vmin + (rect.max.y - p.y) / rect.height() * (vmax - vmin),
        )
    };

    // Sort keys by t for consistent curve drawing
    keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));

    // Draw curve segments
    if keys.len() >= 2 {
        let mut pts = Vec::with_capacity((keys.len() - 1) * 32);
        for w in keys.windows(2) {
            let a = &w[0];
            let b = &w[1];
            let p0 = to_screen(a);
            let p3 = to_screen(b);
            let p1 = p0 + Vec2::new(a.out_tangent.x, -a.out_tangent.y);
            let p2 = p3 + Vec2::new(b.in_tangent.x, -b.in_tangent.y);
            for step in 0..=32 {
                let t = step as f32 / 32.0;
                let omt = 1.0 - t;
                let p = p0.to_vec2() * omt * omt * omt
                    + p1.to_vec2() * 3.0 * omt * omt * t
                    + p2.to_vec2() * 3.0 * omt * t * t
                    + p3.to_vec2() * t * t * t;
                pts.push(p.to_pos2());
            }
        }
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(2.0, theme.widgets.active_bg.to_color32()),
        ));
    }

    // Drag / add / delete keys
    let mut drag_idx = ui.memory(|m| m.data.get_temp::<Option<usize>>(response.id).flatten());
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            drag_idx = keys.iter().position(|k| (to_screen(k) - pos).length() < 8.0);
        }
    }
    if response.dragged() {
        if let Some(idx) = drag_idx {
            if let Some(pos) = response.interact_pointer_pos() {
                let (t, v) = to_curve(pos);
                keys[idx].t = t.clamp(tmin, tmax);
                keys[idx].value = v.clamp(vmin, vmax);
            }
        }
    }
    if response.drag_stopped() {
        drag_idx = None;
    }
    ui.memory_mut(|m| m.data.insert_temp::<Option<usize>>(response.id, drag_idx));

    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(idx) = keys.iter().position(|k| (to_screen(k) - pos).length() < 8.0) {
                keys.remove(idx);
            } else {
                let (t, v) = to_curve(pos);
                keys.push(CurveKey::linear(t.clamp(tmin, tmax), v.clamp(vmin, vmax)));
            }
        }
    }

    // Draw key handles on top
    for k in keys.iter() {
        let p = to_screen(k);
        painter.circle_filled(p, 4.0, theme.widgets.active_bg.to_color32());
        painter.circle_stroke(p, 4.0, Stroke::new(1.0, Color32::WHITE));
    }

    response
}
