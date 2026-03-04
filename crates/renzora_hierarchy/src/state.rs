use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_egui::egui::Color32;
use egui_phosphor::regular;
use renzora_editor::HideInHierarchy;

/// Persistent UI state for the hierarchy panel.
pub struct HierarchyState {
    pub expanded: HashSet<Entity>,
    pub selected: Option<Entity>,
    pub search: String,
    pub show_add_overlay: bool,
    pub add_search: String,
}

impl Default for HierarchyState {
    fn default() -> Self {
        Self {
            expanded: HashSet::new(),
            selected: None,
            search: String::new(),
            show_add_overlay: false,
            add_search: String::new(),
        }
    }
}

/// A node in the entity tree, built each frame from ECS data.
pub struct EntityNode {
    pub entity: Entity,
    pub name: String,
    pub icon: &'static str,
    pub icon_color: Color32,
    pub children: Vec<EntityNode>,
}

/// Build the entity tree from the world.
///
/// Iterates all entities with `Name`, groups them by parent (`ChildOf`),
/// and returns root-level nodes (entities with no parent or whose parent lacks `Name`).
pub fn build_entity_tree(world: &World) -> Vec<EntityNode> {
    // Collect all named entities with their parent info
    let mut entries: Vec<(Entity, String, &'static str, Color32, Option<Entity>)> = Vec::new();
    let mut named_entities: HashSet<Entity> = HashSet::new();

    // Iterate via archetypes (works with &World, no &mut needed)
    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let entity = arch_entity.id();
            let Some(name) = world.get::<Name>(entity) else {
                continue;
            };
            if world.get::<HideInHierarchy>(entity).is_some() {
                continue;
            }
            // Also hide if parent has HideInHierarchy
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                if world.get::<HideInHierarchy>(child_of.parent()).is_some() {
                    continue;
                }
            }
            let name_str = name.as_str().to_string();
            let (icon, color) = entity_icon(world, entity);
            let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            named_entities.insert(entity);
            entries.push((entity, name_str, icon, color, parent));
        }
    }

    // Build children map: parent_entity -> Vec<index into entries>
    let mut children_map: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, (_entity, _name, _icon, _color, parent)) in entries.iter().enumerate() {
        match parent {
            Some(p) if named_entities.contains(p) => {
                children_map.entry(*p).or_default().push(i);
            }
            _ => {
                root_indices.push(i);
            }
        }
    }

    // Sort roots alphabetically
    root_indices.sort_by(|a, b| entries[*a].1.to_lowercase().cmp(&entries[*b].1.to_lowercase()));

    // Recursively build nodes
    fn build_node(
        index: usize,
        entries: &[(Entity, String, &'static str, Color32, Option<Entity>)],
        children_map: &HashMap<Entity, Vec<usize>>,
    ) -> EntityNode {
        let (entity, name, icon, color, _) = &entries[index];
        let mut children = Vec::new();

        if let Some(child_indices) = children_map.get(entity) {
            let mut sorted = child_indices.clone();
            sorted.sort_by(|a, b| {
                entries[*a]
                    .1
                    .to_lowercase()
                    .cmp(&entries[*b].1.to_lowercase())
            });
            for &ci in &sorted {
                children.push(build_node(ci, entries, children_map));
            }
        }

        // If this node has children but used default icon, upgrade to folder
        let final_icon = if !children.is_empty() && *icon == regular::CIRCLE {
            regular::FOLDER
        } else {
            icon
        };
        let final_color = if !children.is_empty() && *icon == regular::CIRCLE {
            Color32::from_rgb(170, 175, 190)
        } else {
            *color
        };

        EntityNode {
            entity: *entity,
            name: name.clone(),
            icon: final_icon,
            icon_color: final_color,
            children,
        }
    }

    root_indices
        .iter()
        .map(|&i| build_node(i, &entries, &children_map))
        .collect()
}

/// Detect an icon and color for an entity based on its components.
fn entity_icon(world: &World, entity: Entity) -> (&'static str, Color32) {
    if world.get::<Camera3d>(entity).is_some() {
        return (regular::VIDEO_CAMERA, Color32::from_rgb(100, 180, 255));
    }
    if world.get::<DirectionalLight>(entity).is_some() {
        return (regular::SUN, Color32::from_rgb(255, 220, 100));
    }
    if world.get::<PointLight>(entity).is_some() {
        return (regular::LIGHTBULB, Color32::from_rgb(255, 200, 80));
    }
    if world.get::<SpotLight>(entity).is_some() {
        return (regular::FLASHLIGHT, Color32::from_rgb(255, 200, 80));
    }
    if world.get::<AmbientLight>(entity).is_some() {
        return (regular::SUN_DIM, Color32::from_rgb(200, 200, 150));
    }
    if world.get::<Mesh3d>(entity).is_some() {
        return (regular::CUBE, Color32::from_rgb(255, 170, 100));
    }
    (regular::CIRCLE, Color32::from_rgb(150, 150, 165))
}

/// Filter the tree to only include nodes whose name matches the search (case-insensitive).
/// A parent is kept if any descendant matches.
pub fn filter_tree(nodes: Vec<EntityNode>, search: &str) -> Vec<EntityNode> {
    let lower = search.to_lowercase();
    nodes
        .into_iter()
        .filter_map(|node| filter_node(node, &lower))
        .collect()
}

fn filter_node(node: EntityNode, search: &str) -> Option<EntityNode> {
    let name_matches = node.name.to_lowercase().contains(search);
    let filtered_children: Vec<EntityNode> = node
        .children
        .into_iter()
        .filter_map(|child| filter_node(child, search))
        .collect();

    if name_matches || !filtered_children.is_empty() {
        Some(EntityNode {
            entity: node.entity,
            name: node.name,
            icon: node.icon,
            icon_color: node.icon_color,
            children: filtered_children,
        })
    } else {
        None
    }
}
