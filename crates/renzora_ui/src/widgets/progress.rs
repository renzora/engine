//! Progress bar, spinner, and skeleton loader — loading-state primitives.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Determinate progress bar. `value` in 0..1.
pub fn progress_bar(ui: &mut egui::Ui, value: f32, height: f32, theme: &Theme) -> egui::Response {
    let w = ui.available_width().max(40.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, height * 0.5, theme.surfaces.faint.to_color32());
    let filled = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * value.clamp(0.0, 1.0), rect.height()),
    );
    p.rect_filled(filled, height * 0.5, theme.widgets.active_bg.to_color32());
    p.rect_stroke(
        rect,
        height * 0.5,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    resp
}

/// Indeterminate spinner — rotating arc.
pub fn spinner(ui: &mut egui::Ui, size: f32, theme: &Theme) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let t = ui.ctx().input(|i| i.time) as f32;
    let center = rect.center();
    let radius = size * 0.4;
    let seg = 24;
    let phase = t * 2.0;
    for i in 0..seg {
        let a = i as f32 / seg as f32 * std::f32::consts::TAU + phase;
        let from = center + Vec2::angled(a) * radius;
        let to = center + Vec2::angled(a) * (radius + size * 0.1);
        let alpha = (i as f32 / seg as f32 * 255.0) as u8;
        let base = theme.widgets.active_bg.to_color32();
        let color = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha);
        ui.painter_at(rect).line_segment([from, to], Stroke::new(2.0, color));
    }
    ui.ctx().request_repaint();
    resp
}

/// Skeleton loader — shimmering rounded rect placeholder.
pub fn skeleton(ui: &mut egui::Ui, width: f32, height: f32, theme: &Theme) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let t = ui.ctx().input(|i| i.time) as f32;
    let phase = (t * 1.5).sin() * 0.5 + 0.5;
    let base = theme.surfaces.faint.to_color32();
    let highlight = theme.widgets.hovered_bg.to_color32();
    let mix = lerp_color(base, highlight, phase);
    ui.painter_at(rect).rect_filled(rect, 4.0, mix);
    ui.ctx().request_repaint();
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| ((x as f32) * (1.0 - t) + (y as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

fn _unused(_: Pos2) {}
