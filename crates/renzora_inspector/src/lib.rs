//! Inspector panel — shows and edits component properties for the selected entity.

mod built_in;
mod field_widget;
mod state;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular;
use renzora_editor::{
    collapsible_section, collapsible_section_removable, empty_state, icon_button, search_overlay,
    EditorCommands, EditorPanel, EditorSelection, InspectorRegistry,
    OverlayAction, OverlayEntry, PanelLocation, PanelRegistry,
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

        let mut state = self._state.write().unwrap();

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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if icon_button(ui, regular::PLUS, "Add Component", theme.semantic.accent.to_color32()) {
                    state.show_add_overlay = true;
                    state.add_search.clear();
                }
            });
        });
        ui.add_space(6.0);

        // Add Component overlay
        if state.show_add_overlay {
            let entries: Vec<OverlayEntry> = registry
                .iter()
                .filter(|e| e.add_fn.is_some() && !(e.has_fn)(world, entity))
                .map(|e| OverlayEntry {
                    id: e.type_id,
                    label: e.display_name,
                    icon: e.icon,
                    category: e.category,
                })
                .collect();

            let ctx = ui.ctx().clone();
            match search_overlay(&ctx, "add_component_overlay", "Add Component", &entries, &mut state.add_search, &theme) {
                OverlayAction::Selected(id) => {
                    state.show_add_overlay = false;
                    if let Some(entry) = registry.iter().find(|e| e.type_id == id) {
                        if let Some(add_fn) = entry.add_fn {
                            cmds.push(move |world: &mut World| {
                                add_fn(world, entity);
                            });
                        }
                    }
                }
                OverlayAction::Closed => {
                    state.show_add_overlay = false;
                }
                OverlayAction::None => {}
            }
        }

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

                    if let Some(remove_fn) = entry.remove_fn {
                        let is_disabled = entry
                            .is_enabled_fn
                            .map(|f| !(f)(world, entity))
                            .unwrap_or(false);

                        let action = collapsible_section_removable(
                            ui,
                            entry.icon,
                            entry.display_name,
                            entry.category,
                            &theme,
                            &format!("inspector_{}", entry.type_id),
                            true,
                            true,
                            is_disabled,
                            |ui| {
                                for (i, field) in entry.fields.iter().enumerate() {
                                    field_widget::render_field(
                                        ui, field, world, entity, cmds, &theme, i,
                                    );
                                }
                            },
                        );
                        if action.remove_clicked {
                            cmds.push(move |world: &mut World| {
                                remove_fn(world, entity);
                            });
                        }
                        if action.toggle_clicked {
                            if let Some(set_enabled_fn) = entry.set_enabled_fn {
                                let new_enabled = is_disabled; // flip: was disabled -> enable
                                cmds.push(move |world: &mut World| {
                                    set_enabled_fn(world, entity, new_enabled);
                                });
                            }
                        }
                    } else {
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
