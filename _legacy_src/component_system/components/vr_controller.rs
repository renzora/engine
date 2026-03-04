//! VR Controller component definition
//!
//! Represents a VR controller attached to an entity. Configures visualization
//! (laser pointer, controller model) and interaction behavior.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::VIRTUAL_REALITY;

pub use renzora_xr::components::VrControllerData;

// ============================================================================
// Custom Add / Remove
// ============================================================================

fn add_vr_controller(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(VrControllerData::default());
}

fn remove_vr_controller(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrControllerData>();
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_vr_controller(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrControllerData>(entity) {
        ui.label("VR Controller Configuration");
        ui.separator();

        // Hand selection
        let mut hand_idx = if data.hand == "right" { 1 } else { 0 };
        if egui::ComboBox::from_label("Hand")
            .show_index(ui, &mut hand_idx, 2, |i| match i {
                0 => "Left",
                _ => "Right",
            })
            .changed()
        {
            data.hand = if hand_idx == 0 { "left".to_string() } else { "right".to_string() };
            changed = true;
        }

        if ui.checkbox(&mut data.show_model, "Show Controller Model").changed() {
            changed = true;
        }
        if ui.checkbox(&mut data.show_laser, "Show Laser Pointer").changed() {
            changed = true;
        }

        if data.show_laser {
            ui.horizontal(|ui| {
                ui.label("Laser Length:");
                if ui.add(egui::DragValue::new(&mut data.laser_length).range(0.5..=20.0).speed(0.1).suffix(" m")).changed() {
                    changed = true;
                }
            });
        }
    }
    changed
}

// ============================================================================
// Scripting Integration
// ============================================================================

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<VrControllerData>(entity) else {
        return vec![];
    };
    vec![
        ("hand", PropertyValue::String(data.hand.clone())),
        ("show_laser", PropertyValue::Bool(data.show_laser)),
        ("show_model", PropertyValue::Bool(data.show_model)),
        ("laser_length", PropertyValue::Float(data.laser_length)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrControllerData>(entity) else {
        return false;
    };
    match prop {
        "hand" => { if let PropertyValue::String(v) = val { data.hand = v.clone(); true } else { false } }
        "show_laser" => { if let PropertyValue::Bool(v) = val { data.show_laser = *v; true } else { false } }
        "show_model" => { if let PropertyValue::Bool(v) = val { data.show_model = *v; true } else { false } }
        "laser_length" => { if let PropertyValue::Float(v) = val { data.laser_length = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("hand", PropertyValueType::String),
        ("show_laser", PropertyValueType::Bool),
        ("show_model", PropertyValueType::Bool),
        ("laser_length", PropertyValueType::Float),
    ]
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrControllerData {
        type_id: "vr_controller",
        display_name: "VR Controller",
        category: ComponentCategory::VR,
        icon: VIRTUAL_REALITY,
        priority: 10,
        custom_inspector: inspect_vr_controller,
        custom_add: add_vr_controller,
        custom_remove: remove_vr_controller,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
