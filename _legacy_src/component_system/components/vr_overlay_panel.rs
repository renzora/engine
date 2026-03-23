//! VR Overlay Panel component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::BROWSER;

pub use renzora_xr::components::VrOverlayPanelData;

fn add_vr_overlay_panel(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrOverlayPanelData::default());
}

fn remove_vr_overlay_panel(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrOverlayPanelData>();
}

fn inspect_vr_overlay_panel(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrOverlayPanelData>(entity) {
        ui.label("VR Overlay Panel");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui.add(egui::DragValue::new(&mut data.width).range(0.1..=5.0).speed(0.01).suffix(" m")).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Pixels/Meter:");
            if ui.add(egui::DragValue::new(&mut data.pixels_per_meter).range(100.0..=4000.0).speed(10.0)).changed() {
                changed = true;
            }
        });

        if ui.checkbox(&mut data.follow_head, "Follow Head").changed() { changed = true; }
        if ui.checkbox(&mut data.curved, "Curved").changed() { changed = true; }

        if data.curved {
            ui.horizontal(|ui| {
                ui.label("Curvature Radius:");
                if ui.add(egui::DragValue::new(&mut data.curvature_radius).range(0.5..=10.0).speed(0.1).suffix(" m")).changed() {
                    changed = true;
                }
            });
        }

        if ui.checkbox(&mut data.interactive, "Interactive").changed() { changed = true; }
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrOverlayPanelData>(entity) else { return vec![] };
    vec![
        ("width", PropertyValue::Float(data.width)),
        ("follow_head", PropertyValue::Bool(data.follow_head)),
        ("curved", PropertyValue::Bool(data.curved)),
        ("interactive", PropertyValue::Bool(data.interactive)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrOverlayPanelData>(entity) else { return false };
    match prop {
        "width" => { if let PropertyValue::Float(v) = val { data.width = *v; true } else { false } }
        "follow_head" => { if let PropertyValue::Bool(v) = val { data.follow_head = *v; true } else { false } }
        "curved" => { if let PropertyValue::Bool(v) = val { data.curved = *v; true } else { false } }
        "interactive" => { if let PropertyValue::Bool(v) = val { data.interactive = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("width", PropertyValueType::Float),
        ("follow_head", PropertyValueType::Bool),
        ("curved", PropertyValueType::Bool),
        ("interactive", PropertyValueType::Bool),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrOverlayPanelData {
        type_id: "vr_overlay_panel",
        display_name: "VR Overlay Panel",
        category: ComponentCategory::VR,
        icon: BROWSER,
        priority: 45,
        custom_inspector: inspect_vr_overlay_panel,
        custom_add: add_vr_overlay_panel,
        custom_remove: remove_vr_overlay_panel,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
