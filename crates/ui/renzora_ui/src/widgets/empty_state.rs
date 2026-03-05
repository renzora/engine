//! Empty-state placeholder — centered icon + message for panels with no content.

use bevy_egui::egui::{self, RichText};
use renzora_theme::Theme;

/// Render a centered empty-state placeholder with a large icon and description.
///
/// ```ignore
/// empty_state(ui, "📂", "No assets", "Drag files here to import.", &theme);
/// ```
pub fn empty_state(ui: &mut egui::Ui, icon: &str, title: &str, description: &str, theme: &Theme) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(32.0).color(theme.text.disabled.to_color32()));
        ui.add_space(8.0);
        ui.label(RichText::new(title).size(14.0).color(theme.text.muted.to_color32()));
        if !description.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(description).size(11.0).color(theme.text.disabled.to_color32()));
        }
    });
}
