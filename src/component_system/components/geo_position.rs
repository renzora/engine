use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::geo_map::data::GeoPositionData;
use crate::ui::property_row;

use egui_phosphor::regular::GLOBE;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_geo_position(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<GeoPositionData>(entity) else {
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
            ui.label("Altitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.altitude).speed(0.1).range(-1000.0..=10000.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Align to Terrain");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.align_to_terrain, "").changed() {
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
    registry.register_owned(register_component!(GeoPositionData {
        type_id: "geo_position",
        display_name: "Geo Position",
        category: ComponentCategory::Gameplay,
        icon: GLOBE,
        custom_inspector: inspect_geo_position,
    }));
}
