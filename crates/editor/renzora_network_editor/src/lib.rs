//! Networking editor panels for the Renzora editor.
//!
//! Panels: Network Monitor, Network Entities, Network Settings.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

// ============================================================================
// Network Monitor Panel
// ============================================================================

struct NetworkMonitorPanel;

impl EditorPanel for NetworkMonitorPanel {
    fn id(&self) -> &str { "network_monitor" }
    fn title(&self) -> &str { "Network Monitor" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::WIFI_HIGH) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::WIFI_SLASH)
                    .size(32.0)
                    .color(muted),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No network connection")
                    .size(13.0)
                    .color(muted),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Configure networking in Network Settings to get started.")
                    .size(11.0)
                    .color(muted),
            );
        });
    }
}

// ============================================================================
// Network Entities Panel
// ============================================================================

struct NetworkEntitiesPanel;

impl EditorPanel for NetworkEntitiesPanel {
    fn id(&self) -> &str { "network_entities" }
    fn title(&self) -> &str { "Network Entities" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::SHARE_NETWORK) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Left }
    fn min_size(&self) -> [f32; 2] { [180.0, 100.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::SHARE_NETWORK)
                    .size(32.0)
                    .color(muted),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No networked entities")
                    .size(13.0)
                    .color(muted),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Add a Networked component to entities\nto see them listed here.")
                    .size(11.0)
                    .color(muted),
            );
        });
    }
}

// ============================================================================
// Network Settings Panel
// ============================================================================

struct NetworkSettingsPanel;

impl EditorPanel for NetworkSettingsPanel {
    fn id(&self) -> &str { "network_settings" }
    fn title(&self) -> &str { "Network Settings" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::GEAR_SIX) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::CLOUD_SLASH)
                    .size(32.0)
                    .color(muted),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Networking not configured")
                    .size(13.0)
                    .color(muted),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Enable [network] in project.toml to configure\nserver mode, transport, and connection settings.")
                    .size(11.0)
                    .color(muted),
            );
        });
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct NetworkEditorPlugin;

impl Plugin for NetworkEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NetworkEditorPlugin");
        app.register_panel(NetworkMonitorPanel);
        app.register_panel(NetworkEntitiesPanel);
        app.register_panel(NetworkSettingsPanel);
    }
}
