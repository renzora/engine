//! Rect editor — two-row Vec2 layout for a Bevy `Rect`.

use bevy::math::Rect;
use bevy_egui::egui;

use super::vector_edit::{vec2_edit, VecEditConfig};

/// Rect editor. Renders `min` on the first row and `max` on the second so that
/// both Vec2 editors get the full row width; aligning them side-by-side is too
/// cramped inside a typical `inline_property` row.
pub fn rect_edit(ui: &mut egui::Ui, value: &mut Rect, cfg: VecEditConfig) -> egui::Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("min").size(10.0));
            vec2_edit(ui, &mut value.min, cfg)
        })
        .inner
        .union(
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("max").size(10.0));
                vec2_edit(ui, &mut value.max, cfg)
            })
            .inner,
        )
    })
    .inner
}
