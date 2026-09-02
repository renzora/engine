//! One-way pushes from [`EditorSettings`] into the subsystems that own the
//! behaviour but can't read the resource themselves.
//!
//! Ember is below this crate in the dependency graph and the console log buffer
//! is a process-global in the contract crate, so neither can look at
//! `EditorSettings` directly. Each of these is change-detected, so it's a no-op
//! on almost every frame.

use bevy::prelude::*;

use renzora_editor_framework::EditorSettings;

/// Push the `EditorSettings.drag_value_rail_sweep` preference into ember's
/// `DragValueConfig` so the numeric-field widget honours the toggle (ember can't
/// read `EditorSettings`). Change-detected, so it's a no-op most frames.
pub(crate) fn sync_drag_value_rail_sweep(
    settings: Res<EditorSettings>,
    mut config: ResMut<renzora_ember::widgets::DragValueConfig>,
) {
    if settings.is_changed() && config.rail_quick_drag != settings.drag_value_rail_sweep {
        config.rail_quick_drag = settings.drag_value_rail_sweep;
    }
}

/// Push the `EditorSettings.scroll_speed` preference into ember's
/// `ScrollConfig` so every scroll gesture (wheel / arrow keys / middle-drag)
/// honours it — same one-way sync as the rail-sweep toggle above.
pub(crate) fn sync_scroll_speed(
    settings: Res<EditorSettings>,
    mut config: ResMut<renzora_ember::widgets::ScrollConfig>,
) {
    if settings.is_changed() && config.speed != settings.scroll_speed {
        config.speed = settings.scroll_speed;
    }
}

/// Push the `EditorSettings.console_log_limit` preference into the shared log
/// buffer's runtime cap so the console retains (and the panel renders) only that
/// many entries. Fires on the first frame (the resource reads changed on insert)
/// so the loaded pref takes effect before much can be logged.
pub(crate) fn sync_console_log_limit(settings: Res<EditorSettings>) {
    if settings.is_changed()
        && renzora::core::console_log::max_log_entries() != settings.console_log_limit
    {
        renzora::core::console_log::set_max_log_entries(settings.console_log_limit);
    }
}
