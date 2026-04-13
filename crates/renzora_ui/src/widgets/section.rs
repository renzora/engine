//! Section header — a styled label used to group controls inside a panel.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Render a small, muted section header with optional spacing.
///
/// ```ignore
/// section_header(ui, "Gravity", &theme);
/// // controls follow ...
/// ```
pub fn section_header(ui: &mut egui::Ui, title: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(title)
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);
}
