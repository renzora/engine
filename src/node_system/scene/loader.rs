use bevy::prelude::*;
use std::path::Path;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::registry::NodeRegistry;

use super::format::{EditorCameraData, NodeData, SceneData};

/// Result of loading a scene - contains camera data and expanded entities
pub struct SceneLoadResult {
    pub editor_camera: EditorCameraData,
    pub expanded_entities: Vec<Entity>,
}

/// Load a scene from a .scene file using the node registry
/// Returns the editor camera data and list of expanded entities
pub fn load_scene(
    path: &Path,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &NodeRegistry,
) -> Result<SceneLoadResult, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let scene_data: SceneData = ron::from_str(&content)?;

    let mut expanded_entities = Vec::new();

    // Spawn root nodes
    for node_data in &scene_data.root_nodes {
        spawn_node(commands, meshes, materials, registry, node_data, None, &mut expanded_entities);
    }

    Ok(SceneLoadResult {
        editor_camera: scene_data.editor_camera,
        expanded_entities,
    })
}

/// Recursively spawn a node and its children from V2 format
pub fn spawn_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &NodeRegistry,
    node_data: &NodeData,
    parent: Option<Entity>,
    expanded_entities: &mut Vec<Entity>,
) -> Entity {
    let transform: Transform = node_data.transform.clone().into();

    // Spawn the base entity with common components
    let mut entity_commands = commands.spawn((
        transform,
        Visibility::default(),
        EditorEntity {
            name: node_data.name.clone(),
        },
        SceneNode,
    ));

    // Add parent relationship if needed
    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    // Look up the node definition and add type-specific components
    if let Some(definition) = registry.get(&node_data.node_type) {
        // Add the node type marker
        entity_commands.insert(NodeTypeMarker::new(definition.type_id));

        // If there's a deserialize function and data, use it
        if let Some(deserialize_fn) = definition.deserialize_fn {
            deserialize_fn(&mut entity_commands, &node_data.data, meshes, materials);
        }
    } else {
        // Unknown node type - just add marker with the type_id from file
        // This preserves unknown types for round-tripping
        warn!("Unknown node type: {}", node_data.node_type);
        // We can't use NodeTypeMarker here since it requires &'static str
        // For now, unknown types become empty nodes
    }

    let entity = entity_commands.id();

    // Track if this entity should be expanded
    if node_data.expanded {
        expanded_entities.push(entity);
    }

    // Spawn children
    for child_data in &node_data.children {
        spawn_node(commands, meshes, materials, registry, child_data, Some(entity), expanded_entities);
    }

    entity
}
