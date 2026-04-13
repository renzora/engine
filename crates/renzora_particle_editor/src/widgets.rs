//! Shared widget helpers for the particle editor.

use renzora::bevy_egui::egui::{self, RichText};
use renzora::theme::Theme;

const LABEL_WIDTH: f32 = 90.0;
const MIN_WIDTH: f32 = 200.0;

/// Inline property row with themed alternating background.
pub fn inline_property<R>(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    theme: &Theme,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let row_even = theme.panels.inspector_row_even.to_color32();
    let row_odd = theme.panels.inspector_row_odd.to_color32();
    let bg_color = if row_index % 2 == 0 { row_even } else { row_odd };
    let available_width = ui.available_width().max(MIN_WIDTH);

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_sized(
                    [LABEL_WIDTH, 16.0],
                    egui::Label::new(RichText::new(label).size(11.0)).truncate(),
                );
                add_widget(ui)
            })
            .inner
        })
        .inner
}
