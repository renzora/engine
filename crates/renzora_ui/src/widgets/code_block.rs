//! Monospaced code block — for console output, shader errors, command
//! previews. No syntax highlighting (wire in syntect later if wanted).

use bevy_egui::egui::{self, Stroke};
use renzora_theme::Theme;

/// Render a selectable monospaced block. Scrolls if content is larger than
/// `max_height`.
pub fn code_block(ui: &mut egui::Ui, text: &str, max_height: f32, theme: &Theme) {
    egui::Frame::new()
        .fill(theme.surfaces.faint.to_color32())
        .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(3)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .monospace()
                                .color(theme.text.primary.to_color32()),
                        )
                        .selectable(true),
                    );
                });
        });
}
