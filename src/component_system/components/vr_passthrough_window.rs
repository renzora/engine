//! VR Passthrough Window component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::FRAME_CORNERS;

pub use renzora_xr::components::VrPassthroughWindowData;

fn add_vr_passthrough_window(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VrPassthroughWindowData::default());
}

fn remove_vr_passthrough_window(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrPassthroughWindowData>();
}

fn inspect_vr_passthrough_window(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrPassthroughWindowData>(entity) {
        ui.label("VR Passthrough Window");
        ui.separator();

        if ui.checkbox(&mut data.enabled, "Enabled").changed() {
            changed = true;
        }

        ui.horizontal(|ui| {
            ui.label("Opacity:");
            if ui.add(egui::Slider::new(&mut data.opacity, 0.0..=1.0)).changed() {
                changed = true;
            }
        });
    }
    changed
}

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrPassthroughWindowData>(entity) else { return vec![] };
    vec![
        ("enabled", PropertyValue::Bool(data.enabled)),
        ("opacity", PropertyValue::Float(data.opacity)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrPassthroughWindowData>(entity) else { return false };
    match prop {
        "enabled" => { if let PropertyValue::Bool(v) = val { data.enabled = *v; true } else { false } }
        "opacity" => { if let PropertyValue::Float(v) = val { data.opacity = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("enabled", PropertyValueType::Bool),
        ("opacity", PropertyValueType::Float),
    ]
}

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrPassthroughWindowData {
        type_id: "vr_passthrough_window",
        display_name: "VR Passthrough Window",
        category: ComponentCategory::VR,
        icon: FRAME_CORNERS,
        priority: 55,
        custom_inspector: inspect_vr_passthrough_window,
        custom_add: add_vr_passthrough_window,
        custom_remove: remove_vr_passthrough_window,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
