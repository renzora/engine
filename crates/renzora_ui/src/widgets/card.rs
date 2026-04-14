//! Card container — themed box with optional header.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Render a card with optional title + body. Pass `None` for `title` to skip
/// the header.
pub fn card<R>(
    ui: &mut egui::Ui,
    title: Option<&str>,
    theme: &Theme,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(theme.surfaces.panel.to_color32())
        .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32()))
        .inner_margin(egui::Margin::same(10))
        .corner_radius(6)
        .show(ui, |ui| {
            if let Some(t) = title {
                ui.label(
                    egui::RichText::new(t)
                        .size(12.0)
                        .strong()
                        .color(theme.text.primary.to_color32()),
                );
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            }
            add_body(ui)
        })
        .inner
}
