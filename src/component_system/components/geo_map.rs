use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::geo_map::data::{GeoMapData, GeoMapAtlas};
use crate::geo_map::style::GeoMapStyle;
use crate::ui::property_row;

use egui_phosphor::regular::GLOBE;

// ============================================================================
// Custom Add/Remove
// ============================================================================

fn add_geo_map(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Insert both the config and the atlas tracker
    commands.entity(entity).insert((
        GeoMapData::default(),
        GeoMapAtlas::default(),
    ));
}

fn remove_geo_map(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<GeoMapData>();
    commands.entity(entity).remove::<GeoMapAtlas>();
    // Child tile entities are removed automatically via ChildOf
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_geo_map(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<GeoMapData>(entity) else {
        return false;
    };
    let mut changed = false;

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Latitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut lat = data.latitude;
                if ui.add(egui::DragValue::new(&mut lat).speed(0.001).range(-90.0..=90.0)).changed() {
                    data.latitude = lat;
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Longitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut lon = data.longitude;
                if ui.add(egui::DragValue::new(&mut lon).speed(0.001).range(-180.0..=180.0)).changed() {
                    data.longitude = lon;
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut zoom = data.zoom as i32;
                if ui.add(egui::Slider::new(&mut zoom, 0..=19)).changed() {
                    data.zoom = zoom as u8;
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tile Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.tile_radius).speed(1.0).range(1..=10)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Style");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let current_label = data.style.label();
                egui::ComboBox::from_id_salt("geo_map_style")
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for style in GeoMapStyle::ALL {
                            if ui.selectable_value(&mut data.style, *style, style.label()).clicked() {
                                if *style != GeoMapStyle::Custom {
                                    data.tile_url_template = style.default_url().to_string();
                                }
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    if data.style == GeoMapStyle::Custom {
        property_row(ui, 5, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL Template");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.text_edit_singleline(&mut data.tile_url_template).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    // Show loading state
    let atlas_info = world.get::<GeoMapAtlas>(entity);
    let (filled, expected) = atlas_info
        .map(|a| (a.tiles_filled, a.tiles_expected))
        .unwrap_or((0, 0));

    property_row(ui, 6, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Refresh Tiles").clicked() {
                // Need to re-borrow mutably
                if let Some(mut d) = world.get_mut::<GeoMapData>(entity) {
                    d.generation += 1;
                }
                changed = true;
            }
            if filled < expected {
                ui.spinner();
                ui.label(format!("{}/{}", filled, expected));
            } else if expected > 0 {
                ui.label(format!("{} tiles", expected));
            }
        });
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(GeoMapData {
        type_id: "geo_map",
        display_name: "Geo Map",
        category: ComponentCategory::Rendering,
        icon: GLOBE,
        priority: 115,
        custom_inspector: inspect_geo_map,
        custom_add: add_geo_map,
        custom_remove: remove_geo_map,
    }));
}
