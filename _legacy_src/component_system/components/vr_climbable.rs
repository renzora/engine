//! VR Climbable surface component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::MOUNTAINS;

pub use renzora_xr::components::VrClimbableData;

fn add_vr_climbable(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrClimbableData::default());
}

fn remove_vr_climbable(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrClimbableData>();
}

fn inspect_vr_climbable(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrClimbableData>(entity) {
        ui.label("VR Climbable");
        ui.separator();

        if ui.checkbox(&mut data.enabled, "Enabled").changed() {
            changed = true;
        }

        ui.horizontal(|ui| {
            ui.label("Grip Distance:");
            if ui.add(egui::DragValue::new(&mut data.grip_distance).range(0.01..=0.5).speed(0.01).suffix(" m")).changed() {
                changed = true;
            }
        });
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrClimbableData>(entity) else { return vec![] };
    vec![
        ("enabled", PropertyValue::Bool(data.enabled)),
        ("grip_distance", PropertyValue::Float(data.grip_distance)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrClimbableData>(entity) else { return false };
    match prop {
        "enabled" => { if let PropertyValue::Bool(v) = val { data.enabled = *v; true } else { false } }
        "grip_distance" => { if let PropertyValue::Float(v) = val { data.grip_distance = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("enabled", PropertyValueType::Bool),
        ("grip_distance", PropertyValueType::Float),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrClimbableData {
        type_id: "vr_climbable",
        display_name: "VR Climbable",
        category: ComponentCategory::VR,
        icon: MOUNTAINS,
        priority: 36,
        custom_inspector: inspect_vr_climbable,
        custom_add: add_vr_climbable,
        custom_remove: remove_vr_climbable,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
