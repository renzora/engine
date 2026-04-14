//! Image viewer with pan and zoom.
//!
//! Caller provides an egui `TextureId` + intrinsic size; the viewer handles
//! pan (drag with middle mouse / shift-drag), zoom (scroll), fit-to-view
//! (double-click).

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Persistent view state for the image viewer. Stored by the caller so
/// state survives between frames.
#[derive(Clone, Copy, Debug)]
pub struct ImageViewerState {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for ImageViewerState {
    fn default() -> Self {
        Self { pan: Vec2::ZERO, zoom: 1.0 }
    }
}

/// Render an image with pan/zoom. Double-click fits the image to the view.
pub fn image_viewer(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    image_size: Vec2,
    state: &mut ImageViewerState,
    theme: &Theme,
) -> egui::Response {
    let w = ui.available_width().max(120.0);
    let h = ui.available_height().max(120.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, theme.surfaces.faint.to_color32());

    // Zoom with scroll
    if response.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            state.zoom = (state.zoom * (1.0 + scroll * 0.001)).clamp(0.05, 20.0);
        }
    }
    // Pan with drag
    if response.dragged() {
        state.pan += response.drag_delta();
    }
    // Fit on double-click
    if response.double_clicked() {
        let fit = (rect.width() / image_size.x).min(rect.height() / image_size.y);
        state.zoom = fit.max(0.05);
        state.pan = Vec2::ZERO;
    }

    // Draw image
    let scaled = image_size * state.zoom;
    let center = rect.center() + state.pan;
    let img_rect = egui::Rect::from_center_size(center, scaled);
    painter.image(
        texture,
        img_rect,
        egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    // Info label
    painter.text(
        Pos2::new(rect.min.x + 6.0, rect.max.y - 6.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{:.0}%", state.zoom * 100.0),
        egui::FontId::proportional(10.0),
        theme.text.muted.to_color32(),
    );

    response
}
