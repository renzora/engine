use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::geo_map::data::GeoMarkerData;
use crate::ui::property_row;

use egui_phosphor::regular::GLOBE;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_geo_marker(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<GeoMarkerData>(entity) else {
        return false;
    };
    let mut changed = false;

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Latitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.latitude).speed(0.0001).range(-90.0..=90.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Longitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.longitude).speed(0.0001).range(-180.0..=180.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Label");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.label).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = data.color;
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    data.color = color;
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scale");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.scale).speed(0.1).range(0.1..=10.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Show Label");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.show_label, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(GeoMarkerData {
        type_id: "geo_marker",
        display_name: "Geo Marker",
        category: ComponentCategory::Gameplay,
        icon: GLOBE,
        custom_inspector: inspect_geo_marker,
    }));
}
