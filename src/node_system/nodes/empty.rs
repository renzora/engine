use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};

/// Node3D (Empty) - basic transform node
pub static NODE3D: NodeDefinition = NodeDefinition {
    type_id: "node.empty",
    display_name: "Node3D (Empty)",
    category: NodeCategory::Nodes3D,
    default_name: "Node3D",
    spawn_fn: spawn_empty_node,
    serialize_fn: None,
    deserialize_fn: None,
    priority: 0,
};

fn spawn_empty_node(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: NODE3D.default_name.to_string(),
        },
        SceneNode,
        NodeTypeMarker::new(NODE3D.type_id),
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
