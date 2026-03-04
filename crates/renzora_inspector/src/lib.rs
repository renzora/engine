//! Inspector panel — shows and edits component properties for the selected entity.

mod built_in;
mod field_widget;
mod state;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular;
use renzora_editor::{
    collapsible_section, empty_state, EditorCommands, EditorPanel, EditorSelection,
    InspectorRegistry, PanelLocation, PanelRegistry,
};
use renzora_theme::ThemeManager;

use state::InspectorState;

/// Inspector panel — displays component fields for the selected entity.
pub struct InspectorPanel {
    _state: RwLock<InspectorState>,
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self {
            _state: RwLock::new(InspectorState::default()),
        }
    }
}

impl EditorPanel for InspectorPanel {
    fn id(&self) -> &str {
        "inspector"
    }

    fn title(&self) -> &str {
        "Inspector"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let selection = world.get_resource::<EditorSelection>();
        let entity = selection.and_then(|s| s.get());

        let Some(entity) = entity else {
            empty_state(
                ui,
                regular::CURSOR_CLICK,
                "No entity selected",
                "Select an entity in the hierarchy to inspect its components.",
                &theme,
            );
            return;
        };

        // Verify entity still exists
        let name = world.get::<Name>(entity);
        if name.is_none() && world.get_entity(entity).is_err() {
            empty_state(
                ui,
                regular::WARNING,
                "Entity not found",
                "The selected entity no longer exists.",
                &theme,
            );
            return;
        }

        let registry = world.get_resource::<InspectorRegistry>();
        let cmds = world.get_resource::<EditorCommands>();

        let (Some(registry), Some(cmds)) = (registry, cmds) else {
            return;
        };

        // Entity header
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let entity_name = name.map(|n| n.as_str()).unwrap_or("Unnamed");
            ui.label(
                RichText::new(entity_name)
                    .size(14.0)
                    .strong()
                    .color(theme.text.heading.to_color32()),
            );
            ui.label(
                RichText::new(format!("({}v{})", entity.index(), entity.generation()))
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            );
        });
        ui.add_space(6.0);

        // Render each component section
        egui::ScrollArea::vertical()
            .id_salt("inspector_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut any_shown = false;

                for entry in registry.iter() {
                    if !(entry.has_fn)(world, entity) {
                        continue;
                    }
                    any_shown = true;

                    collapsible_section(
                        ui,
                        entry.icon,
                        entry.display_name,
                        entry.category,
                        &theme,
                        &format!("inspector_{}", entry.type_id),
                        true,
                        |ui| {
                            for (i, field) in entry.fields.iter().enumerate() {
                                field_widget::render_field(
                                    ui, field, world, entity, cmds, &theme, i,
                                );
                            }
                        },
                    );
                }

                if !any_shown {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("No inspectable components.")
                                .size(11.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    });
                }
            });
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

/// Plugin that registers the `InspectorPanel` and built-in component inspectors.
pub struct InspectorPanelPlugin;

impl Plugin for InspectorPanelPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();

        // Register built-in inspectors
        let mut inspector_registry = world
            .remove_resource::<InspectorRegistry>()
            .unwrap_or_default();
        built_in::register_built_in_inspectors(&mut inspector_registry);
        world.insert_resource(inspector_registry);

        // Register the panel
        let mut panel_registry = world
            .remove_resource::<PanelRegistry>()
            .unwrap_or_default();
        panel_registry.register(InspectorPanel::default());
        world.insert_resource(panel_registry);
    }
}
