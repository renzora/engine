//! Breadcrumb navigation — clickable path segments separated by chevrons.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Render breadcrumbs. Returns the index of the clicked segment, or `None`.
pub fn breadcrumbs(ui: &mut egui::Ui, segments: &[&str], theme: &Theme) -> Option<usize> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, seg) in segments.iter().enumerate() {
            let is_last = i == segments.len() - 1;
            let color = if is_last {
                theme.text.primary.to_color32()
            } else {
                theme.widgets.active_bg.to_color32()
            };
            let resp = ui.selectable_label(
                false,
                egui::RichText::new(*seg).color(color).size(11.0),
            );
            if resp.clicked() && !is_last {
                picked = Some(i);
            }
            if !is_last {
                ui.label(
                    egui::RichText::new("›")
                        .color(theme.text.muted.to_color32())
                        .size(11.0),
                );
            }
        }
    });
    picked
}
