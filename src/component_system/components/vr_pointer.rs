//! VR Pointer (laser ray) component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::CURSOR;

pub use renzora_xr::components::VrPointerData;

fn add_vr_pointer(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrPointerData::default());
}

fn remove_vr_pointer(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrPointerData>();
}

fn inspect_vr_pointer(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrPointerData>(entity) {
        ui.label("VR Pointer");
        ui.separator();

        let mut hand_idx = if data.hand == "right" { 1 } else { 0 };
        if egui::ComboBox::from_label("Hand")
            .show_index(ui, &mut hand_idx, 2, |i| match i { 0 => "Left", _ => "Right" })
            .changed()
        {
            data.hand = if hand_idx == 0 { "left".to_string() } else { "right".to_string() };
            changed = true;
        }

        if ui.checkbox(&mut data.enabled, "Enabled").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Ray Length:");
            if ui.add(egui::DragValue::new(&mut data.ray_length).range(0.5..=50.0).speed(0.1).suffix(" m")).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Ray Width:");
            if ui.add(egui::DragValue::new(&mut data.ray_width).range(0.001..=0.01).speed(0.0005).suffix(" m")).changed() {
                changed = true;
            }
        });

        if ui.checkbox(&mut data.show_cursor, "Show Cursor").changed() { changed = true; }

        if data.show_cursor {
            ui.horizontal(|ui| {
                ui.label("Cursor Size:");
                if ui.add(egui::DragValue::new(&mut data.cursor_size).range(0.005..=0.1).speed(0.001).suffix(" m")).changed() {
                    changed = true;
                }
            });
        }
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrPointerData>(entity) else { return vec![] };
    vec![
        ("hand", PropertyValue::String(data.hand.clone())),
        ("enabled", PropertyValue::Bool(data.enabled)),
        ("ray_length", PropertyValue::Float(data.ray_length)),
        ("show_cursor", PropertyValue::Bool(data.show_cursor)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrPointerData>(entity) else { return false };
    match prop {
        "hand" => { if let PropertyValue::String(v) = val { data.hand = v.clone(); true } else { false } }
        "enabled" => { if let PropertyValue::Bool(v) = val { data.enabled = *v; true } else { false } }
        "ray_length" => { if let PropertyValue::Float(v) = val { data.ray_length = *v; true } else { false } }
        "show_cursor" => { if let PropertyValue::Bool(v) = val { data.show_cursor = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("hand", PropertyValueType::String),
        ("enabled", PropertyValueType::Bool),
        ("ray_length", PropertyValueType::Float),
        ("show_cursor", PropertyValueType::Bool),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrPointerData {
        type_id: "vr_pointer",
        display_name: "VR Pointer",
        category: ComponentCategory::VR,
        icon: CURSOR,
        priority: 16,
        custom_inspector: inspect_vr_pointer,
        custom_add: add_vr_pointer,
        custom_remove: remove_vr_pointer,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
