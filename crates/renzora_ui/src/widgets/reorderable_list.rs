//! Reorderable list — drag a row by its handle to move it up or down.
//!
//! The widget mutates the caller's `Vec<T>` directly by swapping indices on
//! mouse release. Minimal state is stored in egui memory (the dragging
//! index).

use bevy_egui::egui::{self, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Render a reorderable list. `row` is called for every item; `add_handle`
/// decides whether the caller draws the drag-handle themselves (pass false
/// to get the default grip glyph).
pub fn reorderable_list<T>(
    ui: &mut egui::Ui,
    id: egui::Id,
    items: &mut Vec<T>,
    row_height: f32,
    theme: &Theme,
    mut row: impl FnMut(&mut egui::Ui, usize, &mut T),
) {
    let dragging: Option<usize> = ui.memory(|m| m.data.get_temp::<Option<usize>>(id).flatten());
    let mut new_dragging = dragging;
    let mut swap: Option<(usize, usize)> = None;

    for i in 0..items.len() {
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), Sense::click_and_drag());
        let bg = if i % 2 == 0 {
            theme.panels.inspector_row_even.to_color32()
        } else {
            theme.panels.inspector_row_odd.to_color32()
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        // Handle glyph
        let handle_rect = egui::Rect::from_min_size(rect.min, Vec2::new(18.0, row_height));
        ui.painter().text(
            handle_rect.center(),
            egui::Align2::CENTER_CENTER,
            "≡",
            egui::FontId::proportional(13.0),
            theme.text.muted.to_color32(),
        );
        let handle_resp = ui.interact(handle_rect, ui.id().with(("handle", i)), Sense::click_and_drag());
        if handle_resp.drag_started() {
            new_dragging = Some(i);
        }

        // Body
        let body_rect =
            egui::Rect::from_min_max(egui::Pos2::new(rect.min.x + 20.0, rect.min.y), rect.max);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(body_rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| row(ui, i, &mut items[i]),
        );

        // Drop feedback
        if let Some(from) = new_dragging {
            if ui.ctx().pointer_interact_pos().map_or(false, |p| rect.contains(p)) && from != i {
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.5, theme.widgets.active_bg.to_color32()),
                    egui::StrokeKind::Inside,
                );
                if !ui.ctx().input(|inp| inp.pointer.any_down()) {
                    swap = Some((from, i));
                }
            }
        }
        let _ = resp;
    }

    if let Some((from, to)) = swap {
        if from < items.len() && to < items.len() {
            items.swap(from, to);
        }
        new_dragging = None;
    }
    if !ui.ctx().input(|i| i.pointer.any_down()) {
        new_dragging = None;
    }
    ui.memory_mut(|m| m.data.insert_temp::<Option<usize>>(id, new_dragging));
}
