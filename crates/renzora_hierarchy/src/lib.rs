//! Hierarchy panel — shows the scene entity tree.

mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorPanel, EditorSelection, PanelLocation, PanelRegistry};
use renzora_theme::ThemeManager;

use state::{build_entity_tree, filter_tree, HierarchyState};

/// Hierarchy panel — displays all named entities as a tree.
pub struct HierarchyPanel {
    state: RwLock<HierarchyState>,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(HierarchyState::default()),
        }
    }
}

impl EditorPanel for HierarchyPanel {
    fn id(&self) -> &str {
        "hierarchy"
    }

    fn title(&self) -> &str {
        "Hierarchy"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::LIST_BULLETS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // Search bar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .desired_width(ui.available_width() - 8.0)
                    .hint_text(format!("{} Search entities...", regular::MAGNIFYING_GLASS)),
            );
        });
        ui.add_space(4.0);

        // Build entity tree from ECS
        let nodes = build_entity_tree(world);
        let nodes = if state.search.trim().is_empty() {
            nodes
        } else {
            filter_tree(nodes, state.search.trim())
        };

        if nodes.is_empty() {
            let text_muted = theme.text.muted.to_color32();
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No entities in scene.")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        }

        // Render the tree (reborrow to allow split field access)
        let state = &mut *state;
        egui::ScrollArea::vertical()
            .id_salt("hierarchy_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                tree::render_tree(
                    ui,
                    &nodes,
                    &mut state.expanded,
                    &mut state.selected,
                    &theme,
                );
            });

        // Sync local selection → global EditorSelection
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(state.selected);
        }
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
        registry.register(HierarchyPanel::default());
        world.insert_resource(registry);
    }
}
