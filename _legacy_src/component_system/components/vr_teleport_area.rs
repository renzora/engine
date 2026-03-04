//! VR Teleport Area component definition
//!
//! Marks a surface as a valid teleport destination for VR locomotion.
//! Attach to floor/ground entities with physics colliders.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::VIRTUAL_REALITY;

pub use renzora_xr::components::TeleportAreaData;

// ============================================================================
// Custom Add / Remove
// ============================================================================

fn add_teleport_area(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(TeleportAreaData::default());
}

fn remove_teleport_area(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TeleportAreaData>();
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_teleport_area(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<TeleportAreaData>(entity) {
        ui.label("Marks this surface as a valid VR teleport destination.");
        ui.separator();

        if ui.checkbox(&mut data.enabled, "Enabled").changed() {
            changed = true;
        }
        if ui.checkbox(&mut data.restrict_to_bounds, "Restrict to Bounds").changed() {
            changed = true;
        }
    }
    changed
}

// ============================================================================
// Scripting Integration
// ============================================================================

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<TeleportAreaData>(entity) else {
        return vec![];
    };
    vec![
        ("enabled", PropertyValue::Bool(data.enabled)),
        ("restrict_to_bounds", PropertyValue::Bool(data.restrict_to_bounds)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<TeleportAreaData>(entity) else {
        return false;
    };
    match prop {
        "enabled" => { if let PropertyValue::Bool(v) = val { data.enabled = *v; true } else { false } }
        "restrict_to_bounds" => { if let PropertyValue::Bool(v) = val { data.restrict_to_bounds = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("enabled", PropertyValueType::Bool),
        ("restrict_to_bounds", PropertyValueType::Bool),
    ]
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(TeleportAreaData {
        type_id: "vr_teleport_area",
        display_name: "VR Teleport Area",
        category: ComponentCategory::VR,
        icon: VIRTUAL_REALITY,
        priority: 20,
        custom_inspector: inspect_teleport_area,
        custom_add: add_teleport_area,
        custom_remove: remove_teleport_area,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
