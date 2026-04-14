//! Multi-select combobox — ComboBox whose popup shows checkboxes.

use bevy_egui::egui;

/// Multi-select dropdown. Each option has an independent checked state.
/// Returns `true` if any checkbox changed this frame.
pub fn multi_combobox(
    ui: &mut egui::Ui,
    id: egui::Id,
    options: &[&str],
    checked: &mut [bool],
) -> bool {
    let selected_count = checked.iter().filter(|c| **c).count();
    let label = match selected_count {
        0 => "(none)".to_string(),
        1 => {
            let idx = checked.iter().position(|c| *c).unwrap();
            options.get(idx).copied().unwrap_or("?").to_string()
        }
        n => format!("{} selected", n),
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, opt) in options.iter().enumerate() {
                if let Some(v) = checked.get_mut(i) {
                    if ui.checkbox(v, *opt).changed() {
                        changed = true;
                    }
                }
            }
        });
    changed
}
