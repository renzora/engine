//! Resizable split pane — two children separated by a draggable divider.

use bevy_egui::egui::{self, CursorIcon, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

#[derive(Clone, Copy, Debug)]
pub enum PaneSplit {
    Horizontal,
    Vertical,
}

/// Render two panes split by a draggable divider. `split` is a fraction
/// (0.0..1.0) of the first pane's size; mutated by the user's drag.
pub fn split_pane(
    ui: &mut egui::Ui,
    id: egui::Id,
    dir: PaneSplit,
    split: &mut f32,
    theme: &Theme,
    first: impl FnOnce(&mut egui::Ui),
    second: impl FnOnce(&mut egui::Ui),
) {
    let avail = ui.available_size();
    let bar_w = 4.0;
    *split = split.clamp(0.05, 0.95);

    match dir {
        PaneSplit::Horizontal => {
            let total = avail.x;
            let a_w = (total - bar_w) * *split;
            let b_w = total - bar_w - a_w;

            ui.horizontal(|ui| {
                let a_rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(a_w, avail.y));
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(a_rect).layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| first(ui),
                );

                let bar_rect = egui::Rect::from_min_size(
                    Pos2::new(a_rect.max.x, a_rect.min.y),
                    Vec2::new(bar_w, avail.y),
                );
                let bar_resp = ui.interact(bar_rect, id, Sense::drag());
                let bar_color = if bar_resp.hovered() || bar_resp.dragged() {
                    theme.widgets.active_bg.to_color32()
                } else {
                    theme.widgets.border.to_color32()
                };
                ui.painter().rect_filled(bar_rect, 1.0, bar_color);
                if bar_resp.hovered() || bar_resp.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if bar_resp.dragged() {
                    *split += bar_resp.drag_delta().x / total.max(1.0);
                }

                let b_rect = egui::Rect::from_min_size(
                    Pos2::new(bar_rect.max.x, bar_rect.min.y),
                    Vec2::new(b_w, avail.y),
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(b_rect).layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| second(ui),
                );
            });
        }
        PaneSplit::Vertical => {
            let total = avail.y;
            let a_h = (total - bar_w) * *split;
            let b_h = total - bar_w - a_h;
            ui.vertical(|ui| {
                let a_rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(avail.x, a_h));
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(a_rect).layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| first(ui),
                );

                let bar_rect = egui::Rect::from_min_size(
                    Pos2::new(a_rect.min.x, a_rect.max.y),
                    Vec2::new(avail.x, bar_w),
                );
                let bar_resp = ui.interact(bar_rect, id, Sense::drag());
                let bar_color = if bar_resp.hovered() || bar_resp.dragged() {
                    theme.widgets.active_bg.to_color32()
                } else {
                    theme.widgets.border.to_color32()
                };
                ui.painter().rect_filled(bar_rect, 1.0, bar_color);
                if bar_resp.hovered() || bar_resp.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                }
                if bar_resp.dragged() {
                    *split += bar_resp.drag_delta().y / total.max(1.0);
                }

                let b_rect = egui::Rect::from_min_size(
                    Pos2::new(bar_rect.min.x, bar_rect.max.y),
                    Vec2::new(avail.x, b_h),
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(b_rect).layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| second(ui),
                );
            });
        }
    }

    let _ = Stroke::NONE;
}
