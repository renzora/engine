//! Hierarchy panel — shows the scene entity tree.
//!
//! This is a test panel crate that proves the `EditorPanel` registration API works
//! and demonstrates `renzora_widgets` usage.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_editor::{EditorPanel, PanelLocation, PanelRegistry};
use renzora_theme::ThemeManager;

/// Hierarchy panel implementation.
pub struct HierarchyPanel;

impl EditorPanel for HierarchyPanel {
    fn id(&self) -> &str {
        "hierarchy"
    }

    fn title(&self) -> &str {
        "Hierarchy"
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);

        ui.add_space(4.0);

        if let Some(theme) = theme {
            renzora_widgets::section_header(ui, "Scene Entities", theme);
        } else {
            ui.label(egui::RichText::new("Scene Entities").size(12.0));
            ui.add_space(4.0);
        }

        ui.label("No entities in scene.");
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

/// Plugin that registers the `HierarchyPanel` with the editor.
pub struct HierarchyPanelPlugin;

impl Plugin for HierarchyPanelPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();
        let mut registry = world
            .remove_resource::<PanelRegistry>()
            .unwrap_or_default();
        registry.register(HierarchyPanel);
        world.insert_resource(registry);
    }
}
