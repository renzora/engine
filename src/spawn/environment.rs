//! Environment entity spawning

use bevy::prelude::*;
use egui_phosphor::regular::{GLOBE, SPEAKER_HIGH};

use crate::core::{AudioListenerMarker, EditorEntity, NodeIcon, SceneNode, WorldEnvironmentMarker};
use crate::component_system::{
    AmbientLightData,
    SkyboxData, FogData, AntiAliasingData, AmbientOcclusionData,
    ReflectionsData, BloomData, TonemappingData, DepthOfFieldData, MotionBlurData,
};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "World Environment", category: Category::Environment, spawn: spawn_world_environment },
    EntityTemplate { name: "Audio Listener", category: Category::Environment, spawn: spawn_audio_listener },
];

pub fn spawn_world_environment(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "World Environment".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
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

    entity_commands.insert(NodeIcon(GLOBE.to_string()));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_audio_listener(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Audio Listener".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        AudioListenerMarker,
        NodeIcon(SPEAKER_HIGH.to_string()),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
