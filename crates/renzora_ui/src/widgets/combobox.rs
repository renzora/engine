//! Inline enum / variant combobox.
//!
//! Lightweight dropdown for a fixed list of string options. Returns `Some(i)`
//! when the user picks a new index. Designed to fit inside an
//! `inline_property` row; full-screen variant lives in `search_overlay`.

use bevy_egui::egui;

/// Inline dropdown returning the index the user selected this frame, if any.
///
/// `current` is the currently-selected index; callers push a command when
/// `Some(new)` is returned. Safe when `options` is empty (returns `None`).
pub fn enum_combobox(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: usize,
    options: &[&str],
) -> Option<usize> {
    if options.is_empty() {
        ui.label("(no options)");
        return None;
    }
    let current_label = options.get(current).copied().unwrap_or("(invalid)");
    let mut picked: Option<usize> = None;

    egui::ComboBox::from_id_salt(id)
        .selected_text(current_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, label) in options.iter().enumerate() {
                if ui.selectable_label(i == current, *label).clicked() {
                    picked = Some(i);
                }
            }
        });

    picked
}
