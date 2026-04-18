//! Hierarchy tree cache — rebuilds the tree only when ECS changes actually
//! affect it.
//!
//! The panel's `ui()` runs every frame in `EguiPrimaryContextPass` and used
//! to call `build_entity_tree()` unconditionally, which iterates every
//! archetype and walks each entity's ancestor chain. For scenes with
//! thousands of entities this dominated frame time.
//!
//! We now cache the tree in `HierarchyTreeCache` and flip a `HierarchyDirty`
//! flag in a cheap observer system that watches `Added<T>` / `Changed<T>` /
//! `RemovedComponents<T>` for the components the tree actually depends on.
//! The exclusive `update_hierarchy_cache` system runs in `Update`, rebuilds
//! only when dirty, and the panel reads from the cached `Vec<EntityNode>`.

use bevy::prelude::*;
use renzora_editor_framework::{EditorLocked, EntityLabelColor, HideInHierarchy, HierarchyFilter, HierarchyOrder};

use crate::state::{build_entity_tree, EntityNode};

/// Cached entity tree, produced by `update_hierarchy_cache`.
#[derive(Resource, Default)]
pub struct HierarchyTreeCache {
    pub nodes: Vec<EntityNode>,
    /// Monotonic counter; consumers can compare against a stored value to
    /// detect rebuilds without diffing the tree.
    pub version: u64,
}

/// Dirty flag: set by `mark_hierarchy_dirty` whenever a component the tree
/// depends on is added/changed/removed. Cleared by `update_hierarchy_cache`
/// after a successful rebuild.
#[derive(Resource)]
pub struct HierarchyDirty(pub bool);

impl Default for HierarchyDirty {
    // Default-dirty so the first frame populates the cache.
    fn default() -> Self { Self(true) }
}

/// Observe ECS changes that affect the hierarchy tree and flip the dirty
/// flag. Cheap — just iterates filtered queries, doesn't build anything.
pub fn mark_hierarchy_dirty(
    mut dirty: ResMut<HierarchyDirty>,
    filter: Option<Res<HierarchyFilter>>,
    changed_name: Query<(), Or<(Added<Name>, Changed<Name>)>>,
    changed_child_of: Query<(), Changed<ChildOf>>,
    changed_visibility: Query<(), Changed<Visibility>>,
    changed_label: Query<(), Changed<EntityLabelColor>>,
    changed_locked: Query<(), Changed<EditorLocked>>,
    changed_hide: Query<(), Changed<HideInHierarchy>>,
    changed_order: Query<(), Changed<HierarchyOrder>>,
    mut removed_name: RemovedComponents<Name>,
    mut removed_child_of: RemovedComponents<ChildOf>,
    mut removed_hide: RemovedComponents<HideInHierarchy>,
) {
    if dirty.0 { return }

    if filter.as_ref().map_or(false, |f| f.is_changed()) {
        dirty.0 = true;
        return;
    }

    if !changed_name.is_empty()
        || !changed_child_of.is_empty()
        || !changed_visibility.is_empty()
        || !changed_label.is_empty()
        || !changed_locked.is_empty()
        || !changed_hide.is_empty()
        || !changed_order.is_empty()
        || removed_name.read().next().is_some()
        || removed_child_of.read().next().is_some()
        || removed_hide.read().next().is_some()
    {
        dirty.0 = true;
    }
}

/// Exclusive system: rebuilds `HierarchyTreeCache` when dirty. Runs in
/// `Update` so the cache is populated before the egui pass reads it.
pub fn update_hierarchy_cache(world: &mut World) {
    let dirty = world.resource::<HierarchyDirty>().0;
    let empty = world.resource::<HierarchyTreeCache>().nodes.is_empty();
    if !dirty && !empty { return }

    let nodes = build_entity_tree(world);
    let mut cache = world.resource_mut::<HierarchyTreeCache>();
    cache.nodes = nodes;
    cache.version = cache.version.wrapping_add(1);
    world.resource_mut::<HierarchyDirty>().0 = false;
}
