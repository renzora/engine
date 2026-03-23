//! Property row widgets — the foundation of all inspector-style UIs.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Fixed width for property labels.
pub const LABEL_WIDTH: f32 = 80.0;

/// Minimum content width before clipping.
pub const MIN_PANEL_WIDTH: f32 = 220.0;

/// Render a full-width row with alternating themed background.
///
/// `row_index` drives the even/odd color toggle.
pub fn property_row(
    ui: &mut egui::Ui,
    row_index: usize,
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let bg = row_bg(row_index, theme);
    let available_width = ui.available_width();

    egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            add_contents(ui);
        });
}

/// Horizontal layout: fixed-width label on the left, widget on the right, alternating row bg.
///
/// Returns whatever the widget closure returns (e.g. `egui::Response`).
pub fn inline_property<R>(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    theme: &Theme,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let bg = row_bg(row_index, theme);
    let available_width = ui.available_width().max(MIN_PANEL_WIDTH);

    egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // Fixed-width label
                ui.add_sized(
                    [LABEL_WIDTH, 16.0],
                    egui::Label::new(egui::RichText::new(label).size(11.0)).truncate(),
                );

                // Widget fills remaining space
                add_widget(ui)
            })
            .inner
        })
        .inner
}

/// Pick the alternating row background color from the theme.
fn row_bg(row_index: usize, theme: &Theme) -> egui::Color32 {
    if row_index % 2 == 0 {
        theme.panels.inspector_row_even.to_color32()
    } else {
        theme.panels.inspector_row_odd.to_color32()
    }
}
