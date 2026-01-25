//! 2D nodes for 2D scenes

use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};
use crate::shared::{Sprite2DData, Camera2DData};

/// Node2D - empty 2D transform node
pub static NODE2D: NodeDefinition = NodeDefinition {
    type_id: "2d.node2d",
    display_name: "Node2D",
    category: NodeCategory::Nodes2D,
    default_name: "Node2D",
    spawn_fn: spawn_node2d,
    serialize_fn: None,
    deserialize_fn: None,
    priority: 0,
};

fn spawn_node2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: NODE2D.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(NODE2D.type_id),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Sprite2D - displays a 2D sprite/texture
pub static SPRITE2D: NodeDefinition = NodeDefinition {
    type_id: "2d.sprite2d",
    display_name: "Sprite2D",
    category: NodeCategory::Nodes2D,
    default_name: "Sprite2D",
    spawn_fn: spawn_sprite2d,
    serialize_fn: Some(serialize_sprite2d),
    deserialize_fn: Some(deserialize_sprite2d),
    priority: 1,
};

fn spawn_sprite2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let sprite_data = Sprite2DData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: SPRITE2D.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(SPRITE2D.type_id),
        sprite_data,
        // Note: Actual Sprite component is added when texture is set or at runtime
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_sprite2d(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let sprite_data = world.get::<Sprite2DData>(entity)?;
    let mut data = HashMap::new();
    data.insert("texture_path".to_string(), serde_json::json!(sprite_data.texture_path));
    data.insert("color".to_string(), serde_json::json!([
        sprite_data.color.x,
        sprite_data.color.y,
        sprite_data.color.z,
        sprite_data.color.w
    ]));
    data.insert("flip_x".to_string(), serde_json::json!(sprite_data.flip_x));
    data.insert("flip_y".to_string(), serde_json::json!(sprite_data.flip_y));
    data.insert("anchor_x".to_string(), serde_json::json!(sprite_data.anchor.x));
    data.insert("anchor_y".to_string(), serde_json::json!(sprite_data.anchor.y));
    Some(data)
}

fn deserialize_sprite2d(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let texture_path = data
        .get("texture_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let color = data
        .get("color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Vec4::new(
                arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::ONE);

    let flip_x = data.get("flip_x").and_then(|v| v.as_bool()).unwrap_or(false);
    let flip_y = data.get("flip_y").and_then(|v| v.as_bool()).unwrap_or(false);

    let anchor_x = data.get("anchor_x").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    let anchor_y = data.get("anchor_y").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

    entity_commands.insert(Sprite2DData {
        texture_path,
        color,
        flip_x,
        flip_y,
        anchor: Vec2::new(anchor_x, anchor_y),
    });
}

/// Camera2D - 2D orthographic camera
pub static CAMERA2D: NodeDefinition = NodeDefinition {
    type_id: "2d.camera2d",
    display_name: "Camera2D",
    category: NodeCategory::Nodes2D,
    default_name: "Camera2D",
    spawn_fn: spawn_camera2d,
    serialize_fn: Some(serialize_camera2d),
    deserialize_fn: Some(deserialize_camera2d),
    priority: 2,
};

fn spawn_camera2d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let camera_data = Camera2DData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: CAMERA2D.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(CAMERA2D.type_id),
        camera_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_camera2d(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let camera_data = world.get::<Camera2DData>(entity)?;
    let mut data = HashMap::new();
    data.insert("zoom".to_string(), serde_json::json!(camera_data.zoom));
    data.insert("is_default_camera".to_string(), serde_json::json!(camera_data.is_default_camera));
    Some(data)
}

fn deserialize_camera2d(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let zoom = data.get("zoom").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let is_default_camera = data.get("is_default_camera").and_then(|v| v.as_bool()).unwrap_or(false);

    entity_commands.insert(Camera2DData {
        zoom,
        is_default_camera,
    });
}
