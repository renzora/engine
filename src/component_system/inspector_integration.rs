//! Integration between ComponentRegistry and the inspector UI

#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2};

use super::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{PLUS, MAGNIFYING_GLASS, X};

/// State for the Add Component popup
#[derive(Resource, Default)]
pub struct AddComponentPopupState {
    pub is_open: bool,
    pub search_text: String,
    pub selected_category: Option<ComponentCategory>,
}

/// Render the "Add Component" button and popup
pub fn render_add_component_button(
    ui: &mut egui::Ui,
    entity: Entity,
    registry: &ComponentRegistry,
    popup_state: &mut AddComponentPopupState,
    world: &World,
) -> Option<&'static str> {
    let panel_width = ui.available_width();
    let mut component_to_add: Option<&'static str> = None;

    // Add Component button
    if ui
        .add_sized(
            Vec2::new(panel_width - 20.0, 28.0),
            egui::Button::new(RichText::new(format!("{} Add Component", PLUS)).color(Color32::WHITE))
                .fill(Color32::from_rgb(50, 70, 100)),
        )
        .clicked()
    {
        popup_state.is_open = !popup_state.is_open;
        popup_state.search_text.clear();
        popup_state.selected_category = None;
    }

    // Popup
    if popup_state.is_open {
        ui.add_space(4.0);

        egui::Frame::new()
            .fill(Color32::from_rgb(35, 37, 42))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(55, 57, 62)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                // Search bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new(MAGNIFYING_GLASS).color(Color32::GRAY));
                    ui.add(
                        egui::TextEdit::singleline(&mut popup_state.search_text)
                            .hint_text("Search components...")
                            .desired_width(ui.available_width() - 40.0),
                    );
                    if ui
                        .add(egui::Button::new(RichText::new(X).color(Color32::GRAY)).frame(false))
                        .clicked()
                    {
                        popup_state.is_open = false;
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Get available components
                let available = registry.get_available_for(world, entity);

                // Filter by search text
                let search_lower = popup_state.search_text.to_lowercase();
                let filtered: Vec<_> = if search_lower.is_empty() {
                    available.clone()
                } else {
                    available
                        .iter()
                        .filter(|def| {
                            def.display_name.to_lowercase().contains(&search_lower)
                                || def.type_id.contains(&search_lower)
                        })
                        .copied()
                        .collect()
                };

                // Show by category or flat list if searching
                if search_lower.is_empty() {
                    // Show categorized view
                    for category in ComponentCategory::all_in_order() {
                        let in_category: Vec<_> = filtered
                            .iter()
                            .filter(|d| d.category == *category)
                            .copied()
                            .collect();

                        if !in_category.is_empty() {
                            egui::CollapsingHeader::new(
                                RichText::new(format!("{} {}", category.icon(), category.display_name()))
                                    .color(Color32::from_rgb(180, 180, 190)),
                            )
                            .default_open(false)
                            .show(ui, |ui| {
                                for def in in_category {
                                    if render_component_item(ui, def) {
                                        component_to_add = Some(def.type_id);
                                        popup_state.is_open = false;
                                    }
                                }
                            });
                        }
                    }
                } else {
                    // Flat search results
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("No matching components")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                    } else {
                        for def in &filtered {
                            if render_component_item(ui, def) {
                                component_to_add = Some(def.type_id);
                                popup_state.is_open = false;
                            }
                        }
                    }
                }
            });
    }

    component_to_add
}

/// Render a single component item in the popup
fn render_component_item(ui: &mut egui::Ui, def: &ComponentDefinition) -> bool {
    let response = ui.add_sized(
        Vec2::new(ui.available_width(), 24.0),
        egui::Button::new(
            RichText::new(format!("{} {}", def.icon, def.display_name))
                .color(Color32::from_rgb(200, 200, 210)),
        )
        .fill(Color32::TRANSPARENT)
        .frame(false),
    );

    if response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            3.0,
            Color32::from_rgb(55, 60, 70),
        );
    }

    response.clicked()
}

/// Render a component section header with remove button
/// Returns true if the remove button was clicked
pub fn render_component_header_with_remove(
    ui: &mut egui::Ui,
    _icon: &str,
    _label: &str,
    can_remove: bool,
) -> bool {
    let mut remove_clicked = false;

    if can_remove {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new(X).color(Color32::from_rgb(180, 80, 80)).size(10.0))
                        .frame(false),
                )
                .on_hover_text("Remove component")
                .clicked()
            {
                remove_clicked = true;
            }
        });
    }

    remove_clicked
}

/// Get style information for a component category
pub fn get_category_style(category: ComponentCategory) -> (Color32, Color32) {
    match category {
        ComponentCategory::Rendering => (
            Color32::from_rgb(99, 178, 238),   // accent
            Color32::from_rgb(35, 45, 55),     // header bg
        ),
        ComponentCategory::Lighting => (
            Color32::from_rgb(247, 207, 100),
            Color32::from_rgb(50, 45, 35),
        ),
        ComponentCategory::Camera => (
            Color32::from_rgb(178, 132, 209),
            Color32::from_rgb(42, 38, 52),
        ),
        ComponentCategory::Physics => (
            Color32::from_rgb(120, 200, 200),
            Color32::from_rgb(35, 48, 50),
        ),
        ComponentCategory::Audio => (
            Color32::from_rgb(100, 180, 100),
            Color32::from_rgb(35, 45, 40),
        ),
        ComponentCategory::Scripting => (
            Color32::from_rgb(236, 154, 120),
            Color32::from_rgb(50, 40, 38),
        ),
        ComponentCategory::UI => (
            Color32::from_rgb(191, 166, 242),
            Color32::from_rgb(42, 40, 52),
        ),
    }
}
