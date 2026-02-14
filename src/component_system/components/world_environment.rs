//! World Environment — convenience component that adds a full post-processing stack.
//!
//! Adding this component inserts: Ambient Light, Skybox, Fog, Anti-Aliasing,
//! Ambient Occlusion, Reflections, Bloom, Tonemapping, Depth of Field, and Motion Blur.
//! Each can be inspected and removed individually.

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::core::WorldEnvironmentMarker;
use crate::component_system::{
    AmbientLightData,
    SkyboxData, FogData, AntiAliasingData, AmbientOcclusionData,
    ReflectionsData, BloomData, TonemappingData, DepthOfFieldData, MotionBlurData,
};

use egui_phosphor::regular::GLOBE;

// ============================================================================
// World Environment Component (manual definition)
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
    get_script_properties_fn: None,
    set_script_property_fn: None,
    script_property_meta_fn: None,
};

pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&WORLD_ENVIRONMENT);
}

fn add_world_environment(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Convenience group: inserts a marker plus all post-processing components.
    commands.entity(entity).insert((
        WorldEnvironmentMarker,
        AmbientLightData::default(),
        SkyboxData::default(),
        FogData::default(),
        AntiAliasingData::default(),
        AmbientOcclusionData::default(),
        ReflectionsData::default(),
        BloomData::default(),
        TonemappingData::default(),
        DepthOfFieldData::default(),
        MotionBlurData::default(),
    ));
}

fn remove_world_environment(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<WorldEnvironmentMarker>();
}

fn has_world_environment(world: &World, entity: Entity) -> bool {
    world.get::<WorldEnvironmentMarker>(entity).is_some()
}

fn serialize_world_environment(world: &World, entity: Entity) -> Option<serde_json::Value> {
    world.get::<WorldEnvironmentMarker>(entity)?;
    Some(json!({}))
}

fn deserialize_world_environment(
    entity_commands: &mut EntityCommands,
    _data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(WorldEnvironmentMarker);
}

fn inspect_world_environment(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    if world.get::<WorldEnvironmentMarker>(entity).is_none() {
        return false;
    }

    ui.label(
        egui::RichText::new("Convenience group — individual components are shown below.")
            .weak()
            .small(),
    );

    false
}
