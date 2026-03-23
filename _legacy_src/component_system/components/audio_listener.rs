//! Audio Listener component definition
//!
//! Tags an entity as the spatial audio listener. Kira's sync_spatial_audio system
//! reads the transform of this entity to position the listener in 3D space.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::SPEAKER_HIGH;

// ============================================================================
// Data Types
// ============================================================================

#[derive(Component, Default, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AudioListenerData {
    /// Whether this entity is the active listener for 3D spatial audio
    pub active: bool,
}

// ============================================================================
// Custom Add / Remove
// ============================================================================

fn add_audio_listener(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AudioListenerData { active: true });
}

fn remove_audio_listener(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AudioListenerData>();
}

fn deserialize_audio_listener(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(listener) = serde_json::from_value::<AudioListenerData>(data.clone()) {
        entity_commands.insert(listener);
    } else {
        // Backwards-compat: old scenes stored AudioListenerMarker with no data
        entity_commands.insert(AudioListenerData { active: true });
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_audio_listener(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    ui.label("Active listener for 3D spatial audio.");
    ui.label("Attach to the camera or player entity.");

    let mut changed = false;
    if let Some(mut data) = world.get_mut::<AudioListenerData>(entity) {
        if ui.checkbox(&mut data.active, "Active").changed() {
            changed = true;
        }
    }
    changed
}

// ============================================================================
// Scripting Integration
// ============================================================================

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<AudioListenerData>(entity) else {
        return vec![];
    };
    vec![
        ("active", PropertyValue::Bool(data.active)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<AudioListenerData>(entity) else {
        return false;
    };
    match prop {
        "active" => { if let PropertyValue::Bool(v) = val { data.active = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("active", PropertyValueType::Bool),
    ]
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AudioListenerData {
        type_id: "audio_listener",
        display_name: "Audio Listener",
        category: ComponentCategory::Audio,
        icon: SPEAKER_HIGH,
        priority: 0,
        custom_inspector: inspect_audio_listener,
        custom_add: add_audio_listener,
        custom_remove: remove_audio_listener,
        custom_deserialize: deserialize_audio_listener,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
