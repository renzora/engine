//! Hierarchy tree cache — rebuilds the tree only when ECS changes actually
//! affect it.
//!
//! The panel's `ui()` runs every frame in an Update-schedule system and used
//! to call `build_entity_tree()` unconditionally, which iterates every
//! archetype and walks each entity's ancestor chain. For scenes with
//! thousands of entities this dominated frame time.
//!
//! We now cache the tree in `HierarchyTreeCache` and flip a `HierarchyDirty`
//! flag in a cheap observer system that watches `Added<T>` / `Changed<T>` /
//! `RemovedComponents<T>` for the components the tree actually depends on.
//! The exclusive `update_hierarchy_cache` system runs in `Update`, rebuilds
//! only when dirty, and the panel reads from the cached `Vec<EntityNode>`.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use renzora_editor_framework::{
    EditorLocked, EntityLabelColor, HideInHierarchy, HierarchyFilter, HierarchyOrder,
};

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
    fn default() -> Self {
        Self(true)
    }
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
    mut removed_label: RemovedComponents<EntityLabelColor>,
    // Asset badges (script/blueprint/material) ride on these components, so
    // their add/change/remove must rebuild the tree too (grouped into one param
    // to stay under Bevy's per-system param-count cap).
    mut badges: AssetBadgeChanges,
) {
    if dirty.0 {
        return;
    }

    if filter.as_ref().is_some_and(|f| f.is_changed()) {
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
        || removed_label.read().next().is_some()
        || badges.dirty()
    {
        dirty.0 = true;
    }
}

/// Change detection for the components that drive the hierarchy's asset badges,
/// grouped so `mark_hierarchy_dirty` stays under Bevy's system param-count cap.
#[derive(SystemParam)]
pub struct AssetBadgeChanges<'w, 's> {
    // `Changed` already fires on the add tick, so it covers attach + edit.
    changed_script: Query<'w, 's, (), Changed<renzora_scripting::ScriptComponent>>,
    changed_material: Query<'w, 's, (), Changed<renzora::core::MaterialRef>>,
    removed_script: RemovedComponents<'w, 's, renzora_scripting::ScriptComponent>,
    removed_material: RemovedComponents<'w, 's, renzora::core::MaterialRef>,
}

impl AssetBadgeChanges<'_, '_> {
    fn dirty(&mut self) -> bool {
        !self.changed_script.is_empty()
            || !self.changed_material.is_empty()
            || self.removed_script.read().next().is_some()
            || self.removed_material.read().next().is_some()
    }
}

/// Exclusive system: rebuilds `HierarchyTreeCache` when dirty. Runs in
/// `Update` so the cache is populated before the panel reads it.
pub fn update_hierarchy_cache(world: &mut World) {
    let dirty = world.resource::<HierarchyDirty>().0;
    let empty = world.resource::<HierarchyTreeCache>().nodes.is_empty();
    if !dirty && !empty {
        return;
    }

    let nodes = build_entity_tree(world);
    let mut cache = world.resource_mut::<HierarchyTreeCache>();
    cache.nodes = nodes;
    cache.version = cache.version.wrapping_add(1);
    world.resource_mut::<HierarchyDirty>().0 = false;
}
