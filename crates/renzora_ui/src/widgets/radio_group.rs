//! Radio group + checkbox group — compact multi-option selectors.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Vertical radio group. Returns the new index when the user picks a
/// different option; otherwise `None`.
pub fn radio_group(
    ui: &mut egui::Ui,
    options: &[&str],
    selected: usize,
    _theme: &Theme,
) -> Option<usize> {
    let mut picked = None;
    for (i, opt) in options.iter().enumerate() {
        let mut current = i == selected;
        if ui.radio(current, *opt).clicked() {
            current = true;
            picked = Some(i);
        }
        let _ = current;
    }
    picked
}

/// Vertical checkbox group over a boolean slice. Mutates `checked` in place.
/// Returns `true` if any value changed this frame.
pub fn checkbox_group(
    ui: &mut egui::Ui,
    options: &[&str],
    checked: &mut [bool],
    _theme: &Theme,
) -> bool {
    let mut changed = false;
    for (i, opt) in options.iter().enumerate() {
        if let Some(v) = checked.get_mut(i) {
            if ui.checkbox(v, *opt).changed() {
                changed = true;
            }
        }
    }
    changed
}
