//! Renzora Settings — floating overlay window for editor settings.
//!
//! Reads from decentralized resources (`EditorSettings`, `KeyBindings`,
//! `ViewportSettings`, `ThemeManager`) and writes back via direct mutation.
//! The overlay UI is bevy_ui-native (see [`native`]).

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use egui_phosphor::regular::MONITOR;

use renzora_editor::{AppEditorExt, EditorSettings, StatusBarAlignment, StatusBarItem};
use renzora_theme::ThemeManager;

mod native;

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SettingsPlugin");
        app.register_status_item(RendererStatusItem);
        native::build(app);
    }
}

/// Read-only status-bar indicator showing the active graphics backend. Reads
/// the persisted preference (seeded into [`EditorSettings`] at startup) and
/// resolves `Auto` to the concrete backend actually in use.
struct RendererStatusItem;

impl StatusBarItem for RendererStatusItem {
    fn id(&self) -> &str {
        "renderer"
    }

    fn alignment(&self) -> StatusBarAlignment {
        StatusBarAlignment::Right
    }

    fn order(&self) -> i32 {
        -90
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let backend = world
            .get_resource::<EditorSettings>()
            .map(|s| s.renderer_backend)
            .unwrap_or_default()
            .resolved();

        let color = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.text.secondary.to_color32())
            .unwrap_or(Color32::GRAY);

        ui.label(
            RichText::new(format!("{} {}", MONITOR, backend.label()))
                .size(11.0)
                .color(color),
        )
        .on_hover_text("Active graphics backend — change in Settings → Editor → Renderer");
    }
}

renzora::add!(SettingsPlugin, Editor);
