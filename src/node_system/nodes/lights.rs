use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};

/// Point light node
pub static POINT_LIGHT: NodeDefinition = NodeDefinition {
    type_id: "light.point",
    display_name: "Point Light",
    category: NodeCategory::Lights,
    default_name: "Point Light",
    spawn_fn: spawn_point_light,
    serialize_fn: Some(serialize_point_light),
    deserialize_fn: Some(deserialize_point_light),
    priority: 0,
};

/// Directional light node
pub static DIRECTIONAL_LIGHT: NodeDefinition = NodeDefinition {
    type_id: "light.directional",
    display_name: "Directional Light",
    category: NodeCategory::Lights,
    default_name: "Directional Light",
    spawn_fn: spawn_directional_light,
    serialize_fn: Some(serialize_directional_light),
    deserialize_fn: Some(deserialize_directional_light),
    priority: 1,
};

/// Spot light node
pub static SPOT_LIGHT: NodeDefinition = NodeDefinition {
    type_id: "light.spot",
    display_name: "Spot Light",
    category: NodeCategory::Lights,
    default_name: "Spot Light",
    spawn_fn: spawn_spot_light,
    serialize_fn: Some(serialize_spot_light),
    deserialize_fn: Some(deserialize_spot_light),
    priority: 2,
};

// --- Point Light ---

fn spawn_point_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 1000.0,
            range: 20.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 0.0),
        Visibility::default(),
        EditorEntity {
            name: POINT_LIGHT.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(POINT_LIGHT.type_id),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_point_light(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let light = world.get::<PointLight>(entity)?;
    let color = light.color.to_srgba();
    let mut data = HashMap::new();
    data.insert("color".to_string(), serde_json::json!([color.red, color.green, color.blue]));
    data.insert("intensity".to_string(), serde_json::json!(light.intensity));
    data.insert("range".to_string(), serde_json::json!(light.range));
    data.insert("shadows_enabled".to_string(), serde_json::json!(light.shadows_enabled));
    Some(data)
}

fn deserialize_point_light(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.0) as f32;

    let range = data
        .get("range")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    entity_commands.insert(PointLight {
        color,
        intensity,
        range,
        shadows_enabled,
        ..default()
    });
}

// --- Directional Light ---

fn spawn_directional_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
        Visibility::default(),
        EditorEntity {
            name: DIRECTIONAL_LIGHT.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(DIRECTIONAL_LIGHT.type_id),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_directional_light(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let light = world.get::<DirectionalLight>(entity)?;
    let color = light.color.to_srgba();
    let mut data = HashMap::new();
    data.insert("color".to_string(), serde_json::json!([color.red, color.green, color.blue]));
    data.insert("illuminance".to_string(), serde_json::json!(light.illuminance));
    data.insert("shadows_enabled".to_string(), serde_json::json!(light.shadows_enabled));
    Some(data)
}

fn deserialize_directional_light(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let illuminance = data
        .get("illuminance")
        .and_then(|v| v.as_f64())
        .unwrap_or(10000.0) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    entity_commands.insert(DirectionalLight {
        color,
        illuminance,
        shadows_enabled,
        ..default()
    });
}

// --- Spot Light ---

fn spawn_spot_light(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        SpotLight {
            color: Color::WHITE,
            intensity: 1000.0,
            range: 20.0,
            inner_angle: 0.3,
            outer_angle: 0.5,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::default(),
        EditorEntity {
            name: SPOT_LIGHT.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(SPOT_LIGHT.type_id),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_spot_light(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let light = world.get::<SpotLight>(entity)?;
    let color = light.color.to_srgba();
    let mut data = HashMap::new();
    data.insert("color".to_string(), serde_json::json!([color.red, color.green, color.blue]));
    data.insert("intensity".to_string(), serde_json::json!(light.intensity));
    data.insert("range".to_string(), serde_json::json!(light.range));
    data.insert("inner_angle".to_string(), serde_json::json!(light.inner_angle));
    data.insert("outer_angle".to_string(), serde_json::json!(light.outer_angle));
    data.insert("shadows_enabled".to_string(), serde_json::json!(light.shadows_enabled));
    Some(data)
}

fn deserialize_spot_light(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.0) as f32;

    let range = data
        .get("range")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0) as f32;

    let inner_angle = data
        .get("inner_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.3) as f32;

    let outer_angle = data
        .get("outer_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    entity_commands.insert(SpotLight {
        color,
        intensity,
        range,
        inner_angle,
        outer_angle,
        shadows_enabled,
        ..default()
    });
}
