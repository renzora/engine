//! Pixel Brush Settings Panel
//!
//! Brush size, shape, opacity, pixel-perfect toggle, and brush preview.

use bevy_egui::egui::{self, Color32, Rect, Sense, Vec2};
use crate::pixel_editor::{PixelEditorState, BrushShape};
use crate::theming::Theme;

/// Render the pixel brush settings panel
pub fn render_pixel_brush_settings_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let text_color = theme.text.primary.to_color32();

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Brush").size(12.0).color(text_color));
    ui.add_space(4.0);

    // Brush size
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Size:").size(11.0).color(muted));
        let mut size = state.brush_size as i32;
        if ui.add(egui::Slider::new(&mut size, 1..=64).show_value(true)).changed() {
            state.brush_size = size.max(1) as u32;
        }
    });

    ui.add_space(4.0);

    // Brush shape
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Shape:").size(11.0).color(muted));
        if ui.selectable_label(state.brush_shape == BrushShape::Square, "Square").clicked() {
            state.brush_shape = BrushShape::Square;
        }
        if ui.selectable_label(state.brush_shape == BrushShape::Circle, "Circle").clicked() {
            state.brush_shape = BrushShape::Circle;
        }
    });

    ui.add_space(4.0);

    // Opacity
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Opacity:").size(11.0).color(muted));
        let mut opacity_pct = (state.brush_opacity * 100.0) as i32;
        if ui.add(egui::Slider::new(&mut opacity_pct, 0..=100).suffix("%").show_value(true)).changed() {
            state.brush_opacity = opacity_pct as f32 / 100.0;
        }
    });

    ui.add_space(4.0);

    // Pixel perfect
    ui.checkbox(&mut state.pixel_perfect, egui::RichText::new("Pixel Perfect").size(11.0));

    // Grid toggle
    ui.checkbox(&mut state.grid_visible, egui::RichText::new("Show Grid").size(11.0));

    ui.add_space(8.0);
    ui.separator();

    // Brush preview
    ui.label(egui::RichText::new("Preview").size(11.0).color(muted));
    let preview_size = 64.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(preview_size), Sense::hover());

    // Background
    ui.painter().rect_filled(rect, 4.0, Color32::from_gray(25));

    // Draw brush preview
    let center = rect.center();
    let brush_px = state.brush_size as f32;
    let scale = (preview_size - 8.0) / 64.0_f32.max(brush_px);
    let draw_size = brush_px * scale;

    let color = Color32::from_rgba_unmultiplied(
        state.primary_color[0],
        state.primary_color[1],
        state.primary_color[2],
        state.primary_color[3],
    );

    match state.brush_shape {
        BrushShape::Square => {
            let brush_rect = Rect::from_center_size(center, Vec2::splat(draw_size));
            ui.painter().rect_filled(brush_rect, 0.0, color);
        }
        BrushShape::Circle => {
            ui.painter().circle_filled(center, draw_size / 2.0, color);
        }
    }
}
