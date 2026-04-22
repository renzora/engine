//! Property row widgets — the foundation of all inspector-style UIs.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Minimum width reserved for property labels. Labels shorter than this
/// still render at this width so a column of short rows stays aligned.
pub const LABEL_WIDTH: f32 = 80.0;

/// Minimum content width before clipping.
pub const MIN_PANEL_WIDTH: f32 = 220.0;

/// Minimum space reserved for the editor widget on an inline property row.
/// Labels are allowed to grow past `LABEL_WIDTH` up to the point where the
/// widget would fall below this — then the label starts truncating instead.
pub const MIN_WIDGET_WIDTH: f32 = 100.0;

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

                // Grow the label to its natural text width and only fall back
                // to truncation when doing so would push the editor widget
                // below MIN_WIDGET_WIDTH. Floors at LABEL_WIDTH so rows with
                // short labels still align with their neighbours.
                let row_w = ui.available_width();
                let max_label = (row_w - MIN_WIDGET_WIDTH - 2.0).max(LABEL_WIDTH);
                let natural = ui
                    .painter()
                    .layout_no_wrap(
                        label.to_string(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .x;
                // +6 of breathing room so the label never kisses the widget.
                let label_w = (natural + 6.0).min(max_label).max(LABEL_WIDTH);

                ui.add_sized(
                    [label_w, 16.0],
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
