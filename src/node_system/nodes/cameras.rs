use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::{CameraNodeData, NodeTypeMarker};
use crate::node_system::definition::{NodeCategory, NodeDefinition};

/// Camera3D node
pub static CAMERA3D: NodeDefinition = NodeDefinition {
    type_id: "camera.camera3d",
    display_name: "Camera3D",
    category: NodeCategory::Cameras,
    default_name: "Camera3D",
    spawn_fn: spawn_camera,
    serialize_fn: Some(serialize_camera),
    deserialize_fn: Some(deserialize_camera),
    priority: 0,
};

fn spawn_camera(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::default(),
        EditorEntity {
            name: CAMERA3D.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(CAMERA3D.type_id),
        CameraNodeData::default(),
        // Note: We don't add an actual Camera component here
        // as the editor uses its own camera. Scene cameras are
        // for game runtime, stored as metadata for now.
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_camera(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let camera_data = world.get::<CameraNodeData>(entity)?;
    let mut data = HashMap::new();
    data.insert("fov".to_string(), serde_json::json!(camera_data.fov));
    data.insert("is_default_camera".to_string(), serde_json::json!(camera_data.is_default_camera));
    Some(data)
}

fn deserialize_camera(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let fov = data
        .get("fov")
        .and_then(|v| v.as_f64())
        .unwrap_or(45.0) as f32;

    let is_default_camera = data
        .get("is_default_camera")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert(CameraNodeData { fov, is_default_camera });
}
