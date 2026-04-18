//! Badge, chip, keyboard-key pill, avatar, separator.
//!
//! Small display primitives. Kept in one file — each is a handful of lines.

use bevy_egui::egui::{self, Color32, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Tonal badge with optional accent tint. Returns the allocated response.
pub fn badge(ui: &mut egui::Ui, text: &str, accent: Option<Color32>, theme: &Theme) -> egui::Response {
    let base = accent.unwrap_or_else(|| theme.widgets.active_bg.to_color32());
    let bg = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 40);
    let font = egui::FontId::proportional(10.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), base).rect.width());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(text_w + 12.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 8.0, bg);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, base);
    resp
}

/// Keyboard-key pill: "⌘", "Ctrl", "K" etc.
pub fn kbd(ui: &mut egui::Ui, key: &str, theme: &Theme) -> egui::Response {
    let font = egui::FontId::monospace(10.0);
    let text_color = theme.text.primary.to_color32();
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(key.to_string(), font.clone(), text_color).rect.width());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(text_w.max(14.0) + 8.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, theme.surfaces.faint.to_color32());
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, key, font, text_color);
    resp
}

/// Circular avatar placeholder. Renders initials if `image` is `None`.
pub fn avatar(ui: &mut egui::Ui, initials: &str, image: Option<egui::TextureId>, size: f32, theme: &Theme) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if let Some(tex) = image {
        ui.painter().image(
            tex,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().circle_filled(rect.center(), size * 0.5, theme.widgets.active_bg.to_color32());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            initials,
            egui::FontId::proportional(size * 0.4),
            Color32::WHITE,
        );
    }
    resp
}

/// Horizontal divider.
pub fn h_divider(ui: &mut egui::Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, theme.widgets.border.to_color32()));
}

/// Vertical divider.
pub fn v_divider(ui: &mut egui::Ui, height: f32, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter()
        .line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, theme.widgets.border.to_color32()));
}
