//! Inline alert / banner — info, warn, error, success.

use bevy_egui::egui::{self, Color32, Stroke};
use renzora_theme::Theme;

#[derive(Clone, Copy, Debug)]
pub enum AlertKind {
    Info,
    Warn,
    Error,
    Success,
}

/// Render an inline alert banner with an icon + title + body.
pub fn alert(ui: &mut egui::Ui, kind: AlertKind, title: &str, body: &str, theme: &Theme) {
    let (icon, color) = match kind {
        AlertKind::Info => ("ⓘ", Color32::from_rgb(80, 160, 230)),
        AlertKind::Warn => ("⚠", Color32::from_rgb(230, 180, 60)),
        AlertKind::Error => ("✖", Color32::from_rgb(230, 80, 90)),
        AlertKind::Success => ("✓", Color32::from_rgb(90, 190, 120)),
    };
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30);
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, color))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(4)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).size(16.0).color(color));
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .strong()
                            .color(theme.text.primary.to_color32()),
                    );
                    if !body.is_empty() {
                        ui.label(
                            egui::RichText::new(body)
                                .size(11.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    }
                });
            });
        });
}
