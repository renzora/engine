use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_egui::egui::Color32;
use egui_phosphor::regular;
use renzora_blueprint::BlueprintGraph;
use renzora_editor::{EditorLocked, EntityLabelColor, HideInHierarchy, HierarchyFilter, HierarchyOrder};
use renzora_editor::TreeDropZone;

/// Persistent UI state for the hierarchy panel.
pub struct HierarchyState {
    pub expanded: HashSet<Entity>,
    pub search: String,
    pub show_add_overlay: bool,
    pub add_search: String,

    // Drag & drop
    pub drag_entities: Vec<Entity>,
    pub drop_target: Option<(Entity, TreeDropZone)>,

    // Inline rename
    pub renaming_entity: Option<Entity>,
    pub rename_buffer: String,
    pub rename_focus_set: bool,

    // Visible entity order — for Shift+click range selection
    pub visible_entity_order: Vec<Entity>,
    pub building_entity_order: Vec<Entity>,
}

impl Default for HierarchyState {
    fn default() -> Self {
        Self {
            expanded: HashSet::new(),
            search: String::new(),
            show_add_overlay: false,
            add_search: String::new(),
            drag_entities: Vec::new(),
            drop_target: None,
            renaming_entity: None,
            rename_buffer: String::new(),
            rename_focus_set: false,
            visible_entity_order: Vec::new(),
            building_entity_order: Vec::new(),
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
    pub label_color: Option<[u8; 3]>,
    pub is_visible: bool,
    pub is_locked: bool,
    pub is_camera: bool,
    pub is_default_camera: bool,
    pub has_blueprint: bool,
}

/// Build the entity tree from the world.
pub fn build_entity_tree(world: &World) -> Vec<EntityNode> {
    // Resolve hierarchy filter — map component type names to ComponentIds.
    let filter_component_ids: Vec<bevy::ecs::component::ComponentId> = world
        .get_resource::<HierarchyFilter>()
        .and_then(|f| match f {
            HierarchyFilter::All => None,
            HierarchyFilter::OnlyWithComponents(names) => {
                let registry = world.get_resource::<AppTypeRegistry>()?;
                let registry = registry.read();
                Some(
                    names
                        .iter()
                        .filter_map(|name| {
                            // Match by short_path OR by ident (last segment of type path)
                            let reg = registry.iter().find(|r| {
                                let table = r.type_info().type_path_table();
                                table.short_path() == *name || table.ident().map_or(false, |i| i == *name)
                            })?;
                            world.components().get_id(reg.type_id())
                        })
                        .collect(),
                )
            }
        })
        .unwrap_or_default();

    let mut entries: Vec<(Entity, String, &'static str, Color32, Option<Entity>, Option<[u8; 3]>, bool, bool, bool, bool, bool)> = Vec::new();
    let mut named_entities: HashSet<Entity> = HashSet::new();

    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let entity = arch_entity.id();
            let Some(name) = world.get::<Name>(entity) else {
                continue;
            };
            // Apply component filter: skip entities unless they or an ancestor
            // have one of the required components (so children of matching
            // entities still appear in the hierarchy).
            if !filter_component_ids.is_empty() {
                let mut found = false;
                let mut check = entity;
                loop {
                    let er = world.entity(check);
                    if filter_component_ids.iter().any(|id| er.contains_id(*id)) {
                        found = true;
                        break;
                    }
                    match world.get::<ChildOf>(check) {
                        Some(c) => check = c.parent(),
                        None => break,
                    }
                }
                if !found {
                    continue;
                }
            }
            if world.get::<HideInHierarchy>(entity).is_some() {
                continue;
            }
            if world.get::<bevy::input::gamepad::Gamepad>(entity).is_some() {
                continue;
            }
            // Also skip children of gamepad entities (axis/button sub-entities)
            // and any entity whose name indicates it's a system gamepad device.
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                if world.get::<bevy::input::gamepad::Gamepad>(child_of.parent()).is_some() {
                    continue;
                }
            }
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                if world.get::<HideInHierarchy>(child_of.parent()).is_some() {
                    continue;
                }
            }
            let name_str = name.as_str().to_string();
            let (icon, color) = entity_icon(world, entity);
            let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            let label_color = world.get::<EntityLabelColor>(entity).map(|c| c.0);
            let is_visible = world
                .get::<Visibility>(entity)
                .map(|v| *v != Visibility::Hidden)
                .unwrap_or(true);
            let is_locked = world.get::<EditorLocked>(entity).is_some();
            let is_camera = world.get::<Camera3d>(entity).is_some();
            let is_default_camera = world.get::<renzora_core::DefaultCamera>(entity).is_some();
            let has_blueprint = world.get::<BlueprintGraph>(entity).is_some();

            named_entities.insert(entity);
            entries.push((entity, name_str, icon, color, parent, label_color, is_visible, is_locked, is_camera, is_default_camera, has_blueprint));
        }
    }

    let mut children_map: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, &(_, _, _, _, ref parent, _, _, _, _, _, _)) in entries.iter().enumerate() {
        // Walk up the ancestor chain to find the nearest named parent.
        // This handles unnamed intermediaries (e.g. SceneRoot entities in GLTF
        // hierarchies) by reparenting children to the closest visible ancestor.
        let mut resolved_parent = None;
        if let Some(mut p) = *parent {
            loop {
                if named_entities.contains(&p) {
                    resolved_parent = Some(p);
                    break;
                }
                match world.get::<ChildOf>(p) {
                    Some(child_of) => p = child_of.parent(),
                    None => break,
                }
            }
        }
        match resolved_parent {
            Some(p) => {
                children_map.entry(p).or_default().push(i);
            }
            None => {
                root_indices.push(i);
            }
        }
    }

    // Sort root entities by HierarchyOrder component
    root_indices.sort_by_key(|&idx| {
        let entity = entries[idx].0;
        world.get::<HierarchyOrder>(entity).map(|h| h.0).unwrap_or(u32::MAX)
    });

    // Sort children by their order in the parent's Children component
    for (parent_entity, child_indices) in &mut children_map {
        if let Some(children_component) = world.get::<Children>(*parent_entity) {
            let child_order: HashMap<Entity, usize> = children_component
                .iter()
                .enumerate()
                .map(|(pos, e)| (e, pos))
                .collect();
            child_indices.sort_by_key(|&idx| {
                let child_entity = entries[idx].0;
                child_order.get(&child_entity).copied().unwrap_or(usize::MAX)
            });
        }
    }

    fn build_node(
        index: usize,
        entries: &[(Entity, String, &'static str, Color32, Option<Entity>, Option<[u8; 3]>, bool, bool, bool, bool, bool)],
        children_map: &HashMap<Entity, Vec<usize>>,
    ) -> EntityNode {
        let (entity, name, icon, color, _, label_color, is_visible, is_locked, is_camera, is_default_camera, has_blueprint) = &entries[index];
        let mut children = Vec::new();

        if let Some(child_indices) = children_map.get(entity) {
            for &ci in child_indices {
                children.push(build_node(ci, entries, children_map));
            }
        }

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
            label_color: *label_color,
            is_visible: *is_visible,
            is_locked: *is_locked,
            is_camera: *is_camera,
            is_default_camera: *is_default_camera,
            has_blueprint: *has_blueprint,
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
    // UI Canvas
    if world.get::<renzora_game_ui::UiCanvas>(entity).is_some() {
        return (regular::FRAME_CORNERS, Color32::from_rgb(130, 200, 255));
    }
    // UI Widget — use the widget-type-specific icon
    if let Some(widget) = world.get::<renzora_game_ui::UiWidget>(entity) {
        return (widget.widget_type.icon(), Color32::from_rgb(130, 200, 255));
    }
    (regular::CIRCLE, Color32::from_rgb(150, 150, 165))
}

/// Filter the tree to only include nodes whose name matches the search.
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
            children: filtered_children,
            ..node
        })
    } else {
        None
    }
}
