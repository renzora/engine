//! VR Snap Zone component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::MAGNET;

pub use renzora_xr::components::VrSnapZoneData;

fn add_vr_snap_zone(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrSnapZoneData::default());
}

fn remove_vr_snap_zone(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrSnapZoneData>();
}

fn inspect_vr_snap_zone(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrSnapZoneData>(entity) {
        ui.label("VR Snap Zone");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Snap Radius:");
            if ui.add(egui::DragValue::new(&mut data.snap_radius).range(0.01..=1.0).speed(0.01).suffix(" m")).changed() {
                changed = true;
            }
        });

        if ui.checkbox(&mut data.highlight_when_near, "Highlight When Near").changed() {
            changed = true;
        }

        // Occupied indicator (read-only)
        ui.horizontal(|ui| {
            ui.label("Occupied:");
            ui.label(if data.occupied { "Yes" } else { "No" });
        });
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrSnapZoneData>(entity) else { return vec![] };
    vec![
        ("snap_radius", PropertyValue::Float(data.snap_radius)),
        ("highlight_when_near", PropertyValue::Bool(data.highlight_when_near)),
        ("occupied", PropertyValue::Bool(data.occupied)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrSnapZoneData>(entity) else { return false };
    match prop {
        "snap_radius" => { if let PropertyValue::Float(v) = val { data.snap_radius = *v; true } else { false } }
        "highlight_when_near" => { if let PropertyValue::Bool(v) = val { data.highlight_when_near = *v; true } else { false } }
        // occupied is read-only
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("snap_radius", PropertyValueType::Float),
        ("highlight_when_near", PropertyValueType::Bool),
        ("occupied", PropertyValueType::Bool),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrSnapZoneData {
        type_id: "vr_snap_zone",
        display_name: "VR Snap Zone",
        category: ComponentCategory::VR,
        icon: MAGNET,
        priority: 35,
        custom_inspector: inspect_vr_snap_zone,
        custom_add: add_vr_snap_zone,
        custom_remove: remove_vr_snap_zone,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
