//! VR Tracked Object component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::CUBE;

pub use renzora_xr::components::VrTrackedObjectData;

fn add_vr_tracked_object(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrTrackedObjectData::default());
}

fn remove_vr_tracked_object(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrTrackedObjectData>();
}

fn inspect_vr_tracked_object(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrTrackedObjectData>(entity) {
        ui.label("VR Tracked Object");
        ui.separator();

        let roles = ["left_foot", "right_foot", "waist", "chest", "elbow_left", "elbow_right", "knee_left", "knee_right", "camera", "keyboard"];
        let mut role_idx = roles.iter().position(|r| *r == data.tracker_role).unwrap_or(roles.len());
        if egui::ComboBox::from_label("Tracker Role")
            .show_index(ui, &mut role_idx, roles.len() + 1, |i| {
                if i < roles.len() { roles[i] } else { "Custom" }
            })
            .changed()
        {
            if role_idx < roles.len() {
                data.tracker_role = roles[role_idx].to_string();
            }
            changed = true;
        }

        if role_idx >= roles.len() {
            ui.horizontal(|ui| {
                ui.label("Custom Role:");
                if ui.text_edit_singleline(&mut data.tracker_role).changed() {
                    changed = true;
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Serial Number:");
            if ui.text_edit_singleline(&mut data.serial_number).changed() {
                changed = true;
            }
        });

        // Tracked indicator (read-only)
        ui.horizontal(|ui| {
            ui.label("Tracked:");
            ui.label(if data.tracked { "Yes" } else { "No" });
        });
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrTrackedObjectData>(entity) else { return vec![] };
    vec![
        ("tracker_role", PropertyValue::String(data.tracker_role.clone())),
        ("serial_number", PropertyValue::String(data.serial_number.clone())),
        ("tracked", PropertyValue::Bool(data.tracked)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrTrackedObjectData>(entity) else { return false };
    match prop {
        "tracker_role" => { if let PropertyValue::String(v) = val { data.tracker_role = v.clone(); true } else { false } }
        "serial_number" => { if let PropertyValue::String(v) = val { data.serial_number = v.clone(); true } else { false } }
        // tracked is read-only
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("tracker_role", PropertyValueType::String),
        ("serial_number", PropertyValueType::String),
        ("tracked", PropertyValueType::Bool),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrTrackedObjectData {
        type_id: "vr_tracked_object",
        display_name: "VR Tracked Object",
        category: ComponentCategory::VR,
        icon: CUBE,
        priority: 50,
        custom_inspector: inspect_vr_tracked_object,
        custom_add: add_vr_tracked_object,
        custom_remove: remove_vr_tracked_object,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
