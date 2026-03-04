//! Button helpers.

use bevy_egui::egui::{self, Color32, RichText};

/// Frameless icon button with tooltip. Returns `true` if clicked.
///
/// `icon` is a single glyph (Unicode or Phosphor icon string).
pub fn icon_button(ui: &mut egui::Ui, icon: &str, tooltip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(icon).color(color))
            .frame(false),
    )
    .on_hover_text(tooltip)
    .clicked()
}
