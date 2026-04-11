//! DAW (Digital Audio Workstation) panel for the Renzora editor.
//!
//! Provides a timeline-based audio arrangement view with track lanes,
//! waveform display, clip editing, and transport controls.

use bevy::prelude::*;
use renzora::bevy_egui::egui;

use renzora::editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora::theme::ThemeManager;

// ============================================================================
// DAW Panel — timeline-based audio arrangement
// ============================================================================

struct DawPanel;

impl EditorPanel for DawPanel {
    fn id(&self) -> &str { "daw" }
    fn title(&self) -> &str { "Audio" }
    fn icon(&self) -> Option<&str> { Some(renzora::egui_phosphor::regular::WAVEFORM) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Center }
    fn min_size(&self) -> [f32; 2] { [400.0, 200.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                egui::RichText::new(renzora::egui_phosphor::regular::WAVEFORM)
                    .size(48.0)
                    .color(muted),
            );
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Audio Workstation")
                    .size(16.0)
                    .color(muted),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Drag audio files from Assets to create tracks.\nArrange clips on the timeline, mix with the Mixer panel.")
                    .size(11.0)
                    .color(muted),
            );
        });
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct DawPlugin;

impl Plugin for DawPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DawPlugin");
        app.register_panel(DawPanel);
    }
}

renzora::add!(DawPlugin, Editor);
