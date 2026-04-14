//! Swatch palette — grid of saved/recent colors.
//!
//! Click a swatch to pick it. Right-click to remove. Drag-and-drop reorder
//! is left for the caller (pass a mutable `Vec<_>`).

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Render a swatch palette. Returns `Some(index)` when the user clicks a
/// swatch to pick it, or `None` otherwise.
pub fn swatch_palette(
    ui: &mut egui::Ui,
    swatches: &mut Vec<[f32; 4]>,
    swatch_size: f32,
    columns: usize,
    theme: &Theme,
) -> Option<usize> {
    let cols = columns.max(1);
    let rows = (swatches.len() + cols - 1) / cols;
    let spacing = 2.0;
    let total_w = cols as f32 * (swatch_size + spacing) - spacing;
    let total_h = rows as f32 * (swatch_size + spacing) - spacing;

    let (rect, _response) =
        ui.allocate_exact_size(Vec2::new(total_w.max(swatch_size), total_h.max(swatch_size)), Sense::hover());
    let painter = ui.painter_at(rect);
    let mut picked = None;
    let mut to_remove: Option<usize> = None;

    for (i, sw) in swatches.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        let pos = Pos2::new(
            rect.min.x + col as f32 * (swatch_size + spacing),
            rect.min.y + row as f32 * (swatch_size + spacing),
        );
        let cell = egui::Rect::from_min_size(pos, Vec2::splat(swatch_size));
        let color = Color32::from_rgba_unmultiplied(
            (sw[0] * 255.0) as u8,
            (sw[1] * 255.0) as u8,
            (sw[2] * 255.0) as u8,
            (sw[3] * 255.0) as u8,
        );
        painter.rect_filled(cell, 3.0, color);
        painter.rect_stroke(
            cell,
            3.0,
            Stroke::new(1.0, theme.widgets.border.to_color32()),
            egui::StrokeKind::Inside,
        );

        let resp = ui.interact(cell, ui.id().with(("swatch", i)), Sense::click());
        if resp.clicked() {
            picked = Some(i);
        }
        if resp.secondary_clicked() {
            to_remove = Some(i);
        }
    }
    if let Some(i) = to_remove {
        swatches.remove(i);
    }
    picked
}
