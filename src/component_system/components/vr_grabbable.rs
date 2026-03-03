//! VR Grabbable component definition
//!
//! Marks an entity as grabbable by VR controllers. Requires a physics
//! collider for grab detection. Supports snap, offset, and distance grab.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::VIRTUAL_REALITY;

pub use renzora_xr::components::{VrGrabbableData, GrabType};

// ============================================================================
// Custom Add / Remove
// ============================================================================

fn add_vr_grabbable(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(VrGrabbableData::default());
}

fn remove_vr_grabbable(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VrGrabbableData>();
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_vr_grabbable(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VrGrabbableData>(entity) {
        ui.label("Allows this entity to be grabbed by VR controllers.");
        ui.separator();

        // Grab type
        let mut grab_idx = match data.grab_type {
            GrabType::Snap => 0,
            GrabType::Offset => 1,
            GrabType::Distance => 2,
        };
        if egui::ComboBox::from_label("Grab Type")
            .show_index(ui, &mut grab_idx, 3, |i| match i {
                0 => "Snap",
                1 => "Offset",
                _ => "Distance",
            })
            .changed()
        {
            data.grab_type = match grab_idx {
                0 => GrabType::Snap,
                1 => GrabType::Offset,
                _ => GrabType::Distance,
            };
            changed = true;
        }

        if ui.checkbox(&mut data.throwable, "Throwable").changed() {
            changed = true;
        }

        ui.horizontal(|ui| {
            ui.label("Force Multiplier:");
            if ui.add(egui::DragValue::new(&mut data.force_multiplier).range(0.1..=10.0).speed(0.1)).changed() {
                changed = true;
            }
        });

        if data.grab_type == GrabType::Distance {
            ui.horizontal(|ui| {
                ui.label("Max Grab Distance:");
                if ui.add(egui::DragValue::new(&mut data.max_grab_distance).range(0.5..=20.0).speed(0.1).suffix(" m")).changed() {
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
    let Some(data) = world.get::<VrGrabbableData>(entity) else {
        return vec![];
    };
    vec![
        ("throwable", PropertyValue::Bool(data.throwable)),
        ("force_multiplier", PropertyValue::Float(data.force_multiplier)),
        ("max_grab_distance", PropertyValue::Float(data.max_grab_distance)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<VrGrabbableData>(entity) else {
        return false;
    };
    match prop {
        "throwable" => { if let PropertyValue::Bool(v) = val { data.throwable = *v; true } else { false } }
        "force_multiplier" => { if let PropertyValue::Float(v) = val { data.force_multiplier = *v; true } else { false } }
        "max_grab_distance" => { if let PropertyValue::Float(v) = val { data.max_grab_distance = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("throwable", PropertyValueType::Bool),
        ("force_multiplier", PropertyValueType::Float),
        ("max_grab_distance", PropertyValueType::Float),
    ]
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VrGrabbableData {
        type_id: "vr_grabbable",
        display_name: "VR Grabbable",
        category: ComponentCategory::VR,
        icon: VIRTUAL_REALITY,
        priority: 30,
        custom_inspector: inspect_vr_grabbable,
        custom_add: add_vr_grabbable,
        custom_remove: remove_vr_grabbable,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
