use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32};
use egui_phosphor::regular;
use renzora_blueprint::BlueprintGraph;
use renzora_editor::{ComponentIconRegistry, EditorLocked, EntityLabelColor, HideInHierarchy, HierarchyFilter, HierarchyOrder};
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

    /// Selection snapshot from the previous frame. Used to detect selection
    /// changes so we can auto-expand the ancestors of newly selected entities
    /// (otherwise clicking a mesh in the viewport selects a parent that's
    /// collapsed in the tree, and the user sees nothing).
    pub last_selection: Vec<Entity>,

    /// On the next render, nudge the scroll just enough to bring this
    /// entity's row into the visible area. Set whenever the selection
    /// changes (e.g. viewport pick), uses `Align::None` so already-visible
    /// rows don't move the viewport at all.
    pub pending_reveal: Option<Entity>,

    /// Force the tree's scroll back to the top on the next render. Set when
    /// the user edits the search field so filtered results always start
    /// from the top instead of the previous scroll position.
    pub pending_scroll_top: bool,

    // Batch rename
    pub batch_rename_active: bool,
    pub batch_rename_base: String,
    pub batch_rename_start: u32,
    pub batch_rename_entities: Vec<Entity>,

    // Marquee drag selection
    pub marquee_origin: Option<egui::Pos2>,
    pub row_rects: Vec<(Entity, egui::Rect)>,

    /// Per-rendered-row metadata captured by the tree pass and consumed by
    /// the sticky-parent overlay in `lib.rs`. Cleared and rebuilt on each
    /// render. Keeps just enough info to re-paint a row as a sticky header
    /// without re-walking the cache.
    pub row_meta: Vec<StickyRowMeta>,

    /// Filter-by-type — set of registered type names the user wants to show.
    /// Empty means no filter (show everything). The `"__other__"` sentinel
    /// matches entities that don't have a registered type.
    pub type_filter: HashSet<&'static str>,
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
            last_selection: Vec::new(),
            pending_reveal: None,
            pending_scroll_top: false,
            batch_rename_active: false,
            batch_rename_base: String::new(),
            batch_rename_start: 1,
            batch_rename_entities: Vec::new(),
            marquee_origin: None,
            row_rects: Vec::new(),
            row_meta: Vec::new(),
            type_filter: HashSet::new(),
        }
    }
}

/// Data captured per rendered row, used by the sticky-parent overlay so it
/// can re-paint a parent row at the top of the scroll viewport when the
/// original row has scrolled off screen.
#[derive(Clone)]
pub struct StickyRowMeta {
    pub entity: Entity,
    pub rect: egui::Rect,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub name: String,
    pub icon: &'static str,
    pub icon_color: Color32,
    pub label_color: Option<[u8; 3]>,
}

/// A node in the entity tree, built from ECS data. Cached in
/// [`HierarchyTreeCache`] and only rebuilt when the tree actually changes
/// (names, hierarchy, visibility, etc.) — see
/// [`crate::cache::mark_hierarchy_dirty`] / [`crate::cache::update_hierarchy_cache`].
#[derive(Clone)]
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
    pub is_scene_instance: bool,
    /// Registered type label from `ComponentIconRegistry`, or `None` when the
    /// entity didn't match any registered icon entry. Used by the hierarchy's
    /// "filter by type" UI — `None` is grouped under "Other".
    pub type_name: Option<&'static str>,
}

/// Build the entity tree from the world.
pub fn build_entity_tree(world: &World) -> Vec<EntityNode> {
    // Resolve hierarchy filter — map component type names to ComponentIds.
    let resolve_ids = |names: &Vec<&'static str>| -> Vec<bevy::ecs::component::ComponentId> {
        let Some(registry) = world.get_resource::<AppTypeRegistry>() else { return Vec::new(); };
        let registry = registry.read();
        names
            .iter()
            .filter_map(|name| {
                let reg = registry.iter().find(|r| {
                    let table = r.type_info().type_path_table();
                    table.short_path() == *name || table.ident().map_or(false, |i| i == *name)
                })?;
                world.components().get_id(reg.type_id())
            })
            .collect()
    };
    let (include_ids, exclude_ids): (Vec<_>, Vec<_>) = match world.get_resource::<HierarchyFilter>() {
        Some(HierarchyFilter::OnlyWithComponents(names)) => (resolve_ids(names), Vec::new()),
        Some(HierarchyFilter::ExcludeDescendantsOf(names)) => (Vec::new(), resolve_ids(names)),
        _ => (Vec::new(), Vec::new()),
    };
    let filter_component_ids = include_ids;

    struct Entry {
        entity: Entity,
        name: String,
        icon: &'static str,
        color: Color32,
        parent: Option<Entity>,
        label_color: Option<[u8; 3]>,
        is_visible: bool,
        is_locked: bool,
        is_camera: bool,
        is_default_camera: bool,
        has_blueprint: bool,
        is_scene_instance: bool,
        type_name: Option<&'static str>,
    }

    let mut entries: Vec<Entry> = Vec::new();
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
            if !exclude_ids.is_empty() {
                let mut excluded = false;
                let mut check = entity;
                loop {
                    let er = world.entity(check);
                    if exclude_ids.iter().any(|id| er.contains_id(*id)) {
                        excluded = true;
                        break;
                    }
                    match world.get::<ChildOf>(check) {
                        Some(c) => check = c.parent(),
                        None => break,
                    }
                }
                if excluded {
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
            // Children of `HideInHierarchy` parents are NOT auto-hidden — that
            // lets us mark GLTF wrapper nodes (`SceneRoot`, `RootNode.NNN`)
            // hidden so the dropped model's mesh entities promote into the
            // model root rather than appearing under invisible plumbing.
            // Callers that genuinely want to hide a whole subtree mark each
            // descendant individually (see `studio_preview` for the pattern).
            let name_str = name.as_str().to_string();
            let (icon, color) = entity_icon(world, entity);
            let type_name = world
                .get_resource::<ComponentIconRegistry>()
                .and_then(|reg| reg.entity_type_name(world, entity));
            let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            let label_color = world.get::<EntityLabelColor>(entity).map(|c| c.0);
            let is_visible = world
                .get::<Visibility>(entity)
                .map(|v| *v != Visibility::Hidden)
                .unwrap_or(true);
            let is_locked = world.get::<EditorLocked>(entity).is_some();
            let is_camera = world.get::<Camera3d>(entity).is_some();
            let is_default_camera = world.get::<renzora::core::DefaultCamera>(entity).is_some();
            let has_blueprint = world.get::<BlueprintGraph>(entity).is_some();
            let is_scene_instance = world.get::<renzora::SceneInstance>(entity).is_some();

            named_entities.insert(entity);
            entries.push(Entry {
                entity,
                name: name_str,
                icon,
                color,
                parent,
                label_color,
                is_visible,
                is_locked,
                is_camera,
                is_default_camera,
                has_blueprint,
                is_scene_instance,
                type_name,
            });
        }
    }

    let mut children_map: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        // Walk up the ancestor chain to find the nearest named parent.
        // This handles unnamed intermediaries (e.g. SceneRoot entities in GLTF
        // hierarchies) by reparenting children to the closest visible ancestor.
        let mut resolved_parent = None;
        if let Some(mut p) = entry.parent {
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

    // Sort root entities by HierarchyOrder component, using Entity index as
    // tiebreaker so the order is deterministic even when archetype iteration
    // order shifts (e.g. after component additions from selection changes).
    root_indices.sort_by_key(|&idx| {
        let entity = entries[idx].entity;
        let order = world.get::<HierarchyOrder>(entity).map(|h| h.0).unwrap_or(u32::MAX);
        (order, entity)
    });

    // Sort children by a key that's deterministic even when entries were
    // promoted through a hidden ancestor (e.g. a `RootNode_2` wrapper).
    //
    // The sort key for each entry is a path of positions: starting from the
    // entry, walk toward the resolved parent and collect the entry's index
    // inside each direct parent's `Children` component along the way. This
    // preserves the GLB-authored order even when intermediate wrappers are
    // hidden, and is stable across archetype iteration order changes (which
    // shift every frame in play mode and would otherwise scramble promoted
    // siblings here).
    let position_in_parent = |entity: Entity, parent: Entity, world: &World| -> usize {
        world
            .get::<Children>(parent)
            .and_then(|children| children.iter().position(|c| c == entity))
            .unwrap_or(usize::MAX)
    };

    let chain_key = |idx: usize, resolved_parent: Entity, world: &World| -> Vec<usize> {
        let entity = entries[idx].entity;
        let mut path = Vec::new();
        let mut current = entity;
        while current != resolved_parent {
            let Some(direct_parent) = world.get::<ChildOf>(current).map(|c| c.parent()) else {
                break;
            };
            path.push(position_in_parent(current, direct_parent, world));
            current = direct_parent;
        }
        path.reverse();
        path
    };

    for (parent_entity, child_indices) in &mut children_map {
        let parent = *parent_entity;
        // Decorate-sort-undecorate so we don't recompute the chain on every
        // comparison. Tiebreak by Entity for determinism when keys collide
        // (shouldn't happen for valid hierarchies, but cheap insurance).
        let mut keyed: Vec<(Vec<usize>, Entity, usize)> = child_indices
            .iter()
            .map(|&idx| (chain_key(idx, parent, world), entries[idx].entity, idx))
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        *child_indices = keyed.into_iter().map(|(_, _, idx)| idx).collect();
    }

    fn build_node(
        index: usize,
        entries: &[Entry],
        children_map: &HashMap<Entity, Vec<usize>>,
    ) -> EntityNode {
        let entry = &entries[index];
        let mut children = Vec::new();

        if let Some(child_indices) = children_map.get(&entry.entity) {
            for &ci in child_indices {
                children.push(build_node(ci, entries, children_map));
            }
        }

        let final_icon = if !children.is_empty() && entry.icon == regular::CIRCLE {
            regular::FOLDER
        } else {
            entry.icon
        };
        let final_color = if !children.is_empty() && entry.icon == regular::CIRCLE {
            Color32::from_rgb(170, 175, 190)
        } else {
            entry.color
        };

        EntityNode {
            entity: entry.entity,
            name: entry.name.clone(),
            icon: final_icon,
            icon_color: final_color,
            children,
            label_color: entry.label_color,
            is_visible: entry.is_visible,
            is_locked: entry.is_locked,
            is_camera: entry.is_camera,
            is_default_camera: entry.is_default_camera,
            has_blueprint: entry.has_blueprint,
            is_scene_instance: entry.is_scene_instance,
            type_name: entry.type_name,
        }
    }

    root_indices
        .iter()
        .map(|&i| build_node(i, &entries, &children_map))
        .collect()
}

/// Detect an icon and color for an entity using the `ComponentIconRegistry`.
/// Falls back to a generic circle icon if no match is found.
fn entity_icon(world: &World, entity: Entity) -> (&'static str, Color32) {
    if let Some(registry) = world.get_resource::<ComponentIconRegistry>() {
        if let Some((icon, [r, g, b])) = registry.entity_icon(world, entity) {
            return (icon, Color32::from_rgb(r, g, b));
        }
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

/// Filter the tree to only include nodes whose type label is in `allowed`,
/// or whose descendants are. `None`-typed nodes match the sentinel
/// `"__other__"` so the popup can offer an "Other" toggle for entities that
/// don't match any registered type.
pub fn filter_tree_by_type(
    nodes: Vec<EntityNode>,
    allowed: &std::collections::HashSet<&'static str>,
) -> Vec<EntityNode> {
    nodes
        .into_iter()
        .filter_map(|node| filter_node_by_type(node, allowed))
        .collect()
}

fn filter_node_by_type(
    node: EntityNode,
    allowed: &std::collections::HashSet<&'static str>,
) -> Option<EntityNode> {
    let key = node.type_name.unwrap_or("__other__");
    let type_matches = allowed.contains(key);
    let filtered_children: Vec<EntityNode> = node
        .children
        .into_iter()
        .filter_map(|child| filter_node_by_type(child, allowed))
        .collect();

    if type_matches || !filtered_children.is_empty() {
        Some(EntityNode {
            children: filtered_children,
            ..node
        })
    } else {
        None
    }
}
