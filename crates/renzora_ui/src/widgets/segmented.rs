//! Segmented control (pill toggle group) — picks one of N options.

use bevy_egui::egui::{self, Color32, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Render a segmented control. Returns the newly-selected index when the
/// user clicks a different segment.
pub fn segmented_control(
    ui: &mut egui::Ui,
    options: &[&str],
    selected: usize,
    theme: &Theme,
) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let mut picked = None;
    let font = egui::FontId::proportional(11.0);
    let padding = 10.0;
    let h = 22.0;
    let mut widths = Vec::with_capacity(options.len());
    for opt in options {
        let w = ui.fonts_mut(|f| {
            f.layout_no_wrap(opt.to_string(), font.clone(), theme.text.primary.to_color32()).rect.width()
        }) + padding * 2.0;
        widths.push(w);
    }
    let total_w: f32 = widths.iter().sum::<f32>();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, h), Sense::hover());
    ui.painter().rect_filled(rect, h * 0.5, theme.surfaces.faint.to_color32());
    ui.painter().rect_stroke(
        rect,
        h * 0.5,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.min.x;
    for (i, opt) in options.iter().enumerate() {
        let w = widths[i];
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.min.y), Vec2::new(w, h));
        let active = i == selected;
        let resp = ui.interact(cell, ui.id().with(("seg", i)), Sense::click());
        if active {
            ui.painter().rect_filled(cell.shrink(2.0), (h - 4.0) * 0.5, theme.widgets.active_bg.to_color32());
        } else if resp.hovered() {
            ui.painter().rect_filled(cell.shrink(2.0), (h - 4.0) * 0.5, theme.widgets.hovered_bg.to_color32());
        }
        let text_color = if active { Color32::WHITE } else { theme.text.primary.to_color32() };
        ui.painter().text(
            cell.center(),
            egui::Align2::CENTER_CENTER,
            *opt,
            font.clone(),
            text_color,
        );
        if resp.clicked() && !active {
            picked = Some(i);
        }
        x += w;
    }
    picked
}
