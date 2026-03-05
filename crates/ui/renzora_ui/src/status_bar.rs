//! Bottom status bar

use bevy_egui::egui;
use renzora_theme::Theme;

/// Render the status bar at the bottom of the editor window.
pub fn render_status_bar(ctx: &egui::Context, theme: &Theme) {
    egui::TopBottomPanel::bottom("renzora_status_bar")
        .exact_height(22.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new("Ready")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
            });
        });
}
