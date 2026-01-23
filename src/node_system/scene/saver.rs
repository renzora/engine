use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use crate::core::{EditorEntity, SceneNode, SceneTabId, SceneManagerState, HierarchyState, OrbitCameraState};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::registry::NodeRegistry;

use super::format::{EditorCameraData, NodeData, SceneData, TransformData};

/// Collect scene data without saving to file (used for tab switching)
pub fn collect_scene_data(
    scene_name: &str,
    world: &mut World,
    registry: &NodeRegistry,
) -> Result<SceneData, Box<dyn std::error::Error>> {
    // Get current tab and expanded entities
    let (current_tab, expanded_entities) = {
        let scene_state = world.resource::<SceneManagerState>();
        let hierarchy_state = world.resource::<HierarchyState>();
        (scene_state.active_scene_tab, hierarchy_state.expanded_entities.clone())
    };

    // Query for all scene nodes
    let mut query = world.query_filtered::<(
        Entity,
        &Transform,
        &EditorEntity,
        Option<&NodeTypeMarker>,
        Option<&Children>,
        Option<&ChildOf>,
        Option<&SceneTabId>,
    ), With<SceneNode>>();

    // Collect root nodes (entities without parents, belonging to current tab)
    let root_entities: Vec<Entity> = query
        .iter(world)
        .filter(|(_, _, _, _, _, parent, tab_id)| {
            parent.is_none() && tab_id.map_or(false, |t| t.0 == current_tab)
        })
        .map(|(entity, _, _, _, _, _, _)| entity)
        .collect();

    let mut root_nodes = Vec::new();
    for entity in root_entities {
        if let Some(node_data) = entity_to_node_data(entity, world, registry, &expanded_entities) {
            root_nodes.push(node_data);
        }
    }

    // Get editor camera state
    let orbit = world.resource::<OrbitCameraState>();
    let editor_camera = EditorCameraData {
        orbit_focus: [orbit.focus.x, orbit.focus.y, orbit.focus.z],
        orbit_distance: orbit.distance,
        orbit_yaw: orbit.yaw,
        orbit_pitch: orbit.pitch,
    };

    Ok(SceneData {
        name: scene_name.to_string(),
        root_nodes,
        editor_camera,
    })
}

/// Save the current scene to a V2 .scene file using the node registry
/// (Kept for File > Save functionality)
#[allow(dead_code)]
pub fn save_scene(
    path: &Path,
    scene_name: &str,
    world: &mut World,
    registry: &NodeRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    let scene_data = collect_scene_data(scene_name, world, registry)?;

    let content = ron::ser::to_string_pretty(&scene_data, ron::ser::PrettyConfig::default())?;
    std::fs::write(path, content)?;

    Ok(())
}

/// Convert an entity to NodeData recursively
fn entity_to_node_data(
    entity: Entity,
    world: &World,
    registry: &NodeRegistry,
    expanded_entities: &std::collections::HashSet<Entity>,
) -> Option<NodeData> {
    let transform = world.get::<Transform>(entity)?;
    let editor_entity = world.get::<EditorEntity>(entity)?;
    let node_type_marker = world.get::<NodeTypeMarker>(entity);
    let children = world.get::<Children>(entity);

    // Get the node type and serialize type-specific data
    let (node_type, data) = if let Some(marker) = node_type_marker {
        let type_id = marker.type_id.to_string();

        // Try to get the definition and serialize
        let data = if let Some(definition) = registry.get(marker.type_id) {
            if let Some(serialize_fn) = definition.serialize_fn {
                serialize_fn(entity, world).unwrap_or_default()
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        (type_id, data)
    } else {
        // No marker - treat as empty node
        ("node.empty".to_string(), HashMap::new())
    };

    // Check if this entity is expanded in the hierarchy
    let expanded = expanded_entities.contains(&entity);

    // Process children recursively
    let child_nodes: Vec<NodeData> = children
        .map(|c| {
            c.iter()
                .filter_map(|child| entity_to_node_data(child, world, registry, expanded_entities))
                .collect()
        })
        .unwrap_or_default();

    Some(NodeData {
        name: editor_entity.name.clone(),
        transform: TransformData::from(*transform),
        node_type,
        data,
        children: child_nodes,
        expanded,
        visible: editor_entity.visible,
        locked: editor_entity.locked,
    })
}
