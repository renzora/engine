//! Renzora Settings — floating overlay window for editor settings.
//!
//! Reads from decentralized resources (`EditorSettings`, `KeyBindings`,
//! `ViewportSettings`, `ThemeManager`) and writes back via direct mutation.
//! The overlay UI is bevy_ui-native (see [`native`]).

use bevy::prelude::*;

mod native;

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SettingsPlugin");
        native::build(app);
    }
}

renzora::add!(SettingsPlugin, Editor);
