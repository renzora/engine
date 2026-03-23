//! VR Spatial Anchor component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::ANCHOR;

pub use renzora_xr::components::VrSpatialAnchorData;

fn add_vr_spatial_anchor(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrSpatialAnchorData::default());
}

fn remove_vr_spatial_anchor(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrSpatialAnchorData>();
}

fn inspect_vr_spatial_anchor(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrSpatialAnchorData>(entity) {
        ui.label("VR Spatial Anchor");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Anchor ID:");
            if ui.text_edit_singleline(&mut data.anchor_id).changed() {
                changed = true;
            }
        });

        if ui.checkbox(&mut data.persist_across_sessions, "Persist Across Sessions").changed() {
            changed = true;
        }

        // Status (read-only)
        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.label(&data.anchor_status);
        });
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrSpatialAnchorData>(entity) else { return vec![] };
    vec![
        ("anchor_id", PropertyValue::String(data.anchor_id.clone())),
        ("persist_across_sessions", PropertyValue::Bool(data.persist_across_sessions)),
        ("anchor_status", PropertyValue::String(data.anchor_status.clone())),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrSpatialAnchorData>(entity) else { return false };
    match prop {
        "anchor_id" => { if let PropertyValue::String(v) = val { data.anchor_id = v.clone(); true } else { false } }
        "persist_across_sessions" => { if let PropertyValue::Bool(v) = val { data.persist_across_sessions = *v; true } else { false } }
        // anchor_status is read-only
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("anchor_id", PropertyValueType::String),
        ("persist_across_sessions", PropertyValueType::Bool),
        ("anchor_status", PropertyValueType::String),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrSpatialAnchorData {
        type_id: "vr_spatial_anchor",
        display_name: "VR Spatial Anchor",
        category: ComponentCategory::VR,
        icon: ANCHOR,
        priority: 40,
        custom_inspector: inspect_vr_spatial_anchor,
        custom_add: add_vr_spatial_anchor,
        custom_remove: remove_vr_spatial_anchor,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
