//! Environment component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::core::WorldEnvironmentMarker;
use crate::shared::WorldEnvironmentData;

use egui_phosphor::regular::GLOBE;

// ============================================================================
// World Environment Component
// ============================================================================

pub static WORLD_ENVIRONMENT: ComponentDefinition = ComponentDefinition {
    type_id: "world_environment",
    display_name: "World Environment",
    category: ComponentCategory::Rendering,
    icon: GLOBE,
    priority: 100,
    add_fn: add_world_environment,
    remove_fn: remove_world_environment,
    has_fn: has_world_environment,
    serialize_fn: serialize_world_environment,
    deserialize_fn: deserialize_world_environment,
    inspector_fn: inspect_world_environment,
    conflicts_with: &[],
    requires: &[],
};

/// Register all environment components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&WORLD_ENVIRONMENT);
}

fn add_world_environment(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(WorldEnvironmentMarker {
        data: WorldEnvironmentData::default(),
    });
}

fn remove_world_environment(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<WorldEnvironmentMarker>();
}

fn has_world_environment(world: &World, entity: Entity) -> bool {
    world.get::<WorldEnvironmentMarker>(entity).is_some()
}

fn serialize_world_environment(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let marker = world.get::<WorldEnvironmentMarker>(entity)?;
    Some(json!({
        "data": marker.data
    }))
}

fn deserialize_world_environment(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let env_data = data.get("data")
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    entity_commands.insert(WorldEnvironmentMarker {
        data: env_data,
    });
}

fn inspect_world_environment(
    ui: &mut egui::Ui,
    _world: &mut World,
    _entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    ui.label("World environment settings.");
    ui.label("Controls ambient light, sky, fog, and post-processing.");
    false
}
