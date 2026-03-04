//! Toggle switch — animated pill-shaped on/off control.

use bevy_egui::egui::{self, Color32, CursorIcon, Sense, Vec2};

/// Paint an animated toggle switch. Returns `true` if the switch was clicked.
///
/// The switch animates between off (grey) and on (green) over 150ms.
pub fn toggle_switch(ui: &mut egui::Ui, id: egui::Id, enabled: bool) -> bool {
    let size = Vec2::new(28.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_with_time(id, enabled, 0.15);

        // Background: grey → green
        let bg = Color32::from_rgb(
            (80.0 + (89.0 - 80.0) * t) as u8,
            (80.0 + (191.0 - 80.0) * t) as u8,
            (85.0 + (115.0 - 85.0) * t) as u8,
        );

        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, bg);

        // Knob
        let knob_r = radius - 2.0;
        let knob_x = rect.left() + radius + t * (rect.width() - 2.0 * radius);
        ui.painter()
            .circle_filled(egui::pos2(knob_x, rect.center().y), knob_r, Color32::WHITE);
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    response.clicked()
}
