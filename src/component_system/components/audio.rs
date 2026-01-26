//! Audio component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

// ============================================================================
// Audio Listener Component (marker only for now)
// ============================================================================

/// Marker component for audio listener
#[derive(Component, Default, Clone)]
pub struct AudioListenerMarker;

pub static AUDIO_LISTENER: ComponentDefinition = ComponentDefinition {
    type_id: "audio_listener",
    display_name: "Audio Listener",
    category: ComponentCategory::Audio,
    icon: "\u{e9ce}", // Volume icon
    priority: 0,
    add_fn: add_audio_listener,
    remove_fn: remove_audio_listener,
    has_fn: has_audio_listener,
    serialize_fn: serialize_audio_listener,
    deserialize_fn: deserialize_audio_listener,
    inspector_fn: inspect_audio_listener,
    conflicts_with: &[],
    requires: &[],
};

/// Register all audio components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&AUDIO_LISTENER);
}

fn add_audio_listener(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AudioListenerMarker);
}

fn remove_audio_listener(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AudioListenerMarker>();
}

fn has_audio_listener(world: &World, entity: Entity) -> bool {
    world.get::<AudioListenerMarker>(entity).is_some()
}

fn serialize_audio_listener(_world: &World, _entity: Entity) -> Option<serde_json::Value> {
    Some(json!({}))
}

fn deserialize_audio_listener(
    entity_commands: &mut EntityCommands,
    _data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(AudioListenerMarker);
}

fn inspect_audio_listener(
    ui: &mut egui::Ui,
    _world: &mut World,
    _entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    ui.label("Audio listener for 3D spatial audio.");
    ui.label("Attach to the player or camera.");
    false
}
