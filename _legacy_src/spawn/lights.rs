//! Light entity spawning

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::component_system::{DirectionalLightData, PointLightData, SpotLightData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Point Light", category: Category::Light, spawn: spawn_point_light },
    EntityTemplate { name: "Directional Light", category: Category::Light, spawn: spawn_directional_light },
    EntityTemplate { name: "Spot Light", category: Category::Light, spawn: spawn_spot_light },
];

pub fn spawn_point_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let data = PointLightData::default();

    let mut entity_commands = commands.spawn((
        // Bevy light component for editor rendering
        PointLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        // Data component for serialization
        data,
        Transform::from_xyz(0.0, 3.0, 0.0),
        Visibility::default(),
        EditorEntity {
            name: "Point Light".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_directional_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let data = DirectionalLightData::default();

    let mut entity_commands = commands.spawn((
        // Bevy light component for editor rendering
        DirectionalLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            illuminance: data.illuminance,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        // Data component for serialization
        data,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
        Visibility::default(),
        EditorEntity {
            name: "Directional Light".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_spot_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let data = SpotLightData::default();

    let mut entity_commands = commands.spawn((
        // Bevy light component for editor rendering
        SpotLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            inner_angle: data.inner_angle,
            outer_angle: data.outer_angle,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        // Data component for serialization
        data,
        Transform::from_xyz(0.0, 5.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::default(),
        EditorEntity {
            name: "Spot Light".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
