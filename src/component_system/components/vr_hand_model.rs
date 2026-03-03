//! VR Hand Model component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::HAND;

pub use renzora_xr::components::VrHandModelData;

fn add_vr_hand_model(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrHandModelData::default());
}

fn remove_vr_hand_model(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrHandModelData>();
}

fn inspect_vr_hand_model(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrHandModelData>(entity) {
        ui.label("VR Hand Model");
        ui.separator();

        // Hand selection
        let mut hand_idx = if data.hand == "right" { 1 } else { 0 };
        if egui::ComboBox::from_label("Hand")
            .show_index(ui, &mut hand_idx, 2, |i| match i { 0 => "Left", _ => "Right" })
            .changed()
        {
            data.hand = if hand_idx == 0 { "left".to_string() } else { "right".to_string() };
            changed = true;
        }

        // Model type
        let model_types = ["controller", "hand", "custom"];
        let mut type_idx = model_types.iter().position(|t| *t == data.model_type).unwrap_or(0);
        if egui::ComboBox::from_label("Model Type")
            .show_index(ui, &mut type_idx, 3, |i| match i { 0 => "Controller", 1 => "Hand", _ => "Custom" })
            .changed()
        {
            data.model_type = model_types[type_idx].to_string();
            changed = true;
        }

        if data.model_type == "custom" {
            ui.horizontal(|ui| {
                ui.label("Mesh Path:");
                if ui.text_edit_singleline(&mut data.custom_mesh).changed() {
                    changed = true;
                }
            });
        }

        if ui.checkbox(&mut data.visible, "Visible").changed() {
            changed = true;
        }
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrHandModelData>(entity) else { return vec![] };
    vec![
        ("hand", PropertyValue::String(data.hand.clone())),
        ("model_type", PropertyValue::String(data.model_type.clone())),
        ("visible", PropertyValue::Bool(data.visible)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrHandModelData>(entity) else { return false };
    match prop {
        "hand" => { if let PropertyValue::String(v) = val { data.hand = v.clone(); true } else { false } }
        "model_type" => { if let PropertyValue::String(v) = val { data.model_type = v.clone(); true } else { false } }
        "visible" => { if let PropertyValue::Bool(v) = val { data.visible = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("hand", PropertyValueType::String),
        ("model_type", PropertyValueType::String),
        ("visible", PropertyValueType::Bool),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrHandModelData {
        type_id: "vr_hand_model",
        display_name: "VR Hand Model",
        category: ComponentCategory::VR,
        icon: HAND,
        priority: 15,
        custom_inspector: inspect_vr_hand_model,
        custom_add: add_vr_hand_model,
        custom_remove: remove_vr_hand_model,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
