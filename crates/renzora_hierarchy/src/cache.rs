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
    EditorLocked, EntityIcon, EntityLabelColor, HideInHierarchy, HierarchyFilter, HierarchyOrder,
};

use crate::state::{build_entity_tree, EntityNode, HierarchySpawnSeq};

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
    changed_locked: Query<(), Changed<EditorLocked>>,
    changed_hide: Query<(), Changed<HideInHierarchy>>,
    changed_order: Query<(), Changed<HierarchyOrder>>,
    mut removed_name: RemovedComponents<Name>,
    mut removed_child_of: RemovedComponents<ChildOf>,
    mut removed_hide: RemovedComponents<HideInHierarchy>,
    // The label colour and icon override, grouped for the same reason as
    // `AssetBadgeChanges` below — four more bare params would push this system
    // past Bevy's per-system cap.
    mut identity: IdentityChanges,
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
        || !changed_locked.is_empty()
        || !changed_hide.is_empty()
        || !changed_order.is_empty()
        || removed_name.read().next().is_some()
        || removed_child_of.read().next().is_some()
        || removed_hide.read().next().is_some()
        || identity.dirty()
        || badges.dirty()
    {
        dirty.0 = true;
    }
}

/// Change detection for the entity's *authored* identity — the label colour and
/// the icon override the inspector's entity header edits. Both are set and
/// cleared from outside the hierarchy, so without the removal halves an entity
/// reset to "Auto" would keep drawing its old icon until something unrelated
/// dirtied the tree.
#[derive(SystemParam)]
pub struct IdentityChanges<'w, 's> {
    changed_label: Query<'w, 's, (), Changed<EntityLabelColor>>,
    changed_icon: Query<'w, 's, (), Changed<EntityIcon>>,
    removed_label: RemovedComponents<'w, 's, EntityLabelColor>,
    removed_icon: RemovedComponents<'w, 's, EntityIcon>,
}

impl IdentityChanges<'_, '_> {
    fn dirty(&mut self) -> bool {
        !self.changed_label.is_empty()
            || !self.changed_icon.is_empty()
            || self.removed_label.read().next().is_some()
            || self.removed_icon.read().next().is_some()
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
pub fn update_hierarchy_cache(world: &mut World, mut last_build: Local<f32>) {
    let dirty = world.resource::<HierarchyDirty>().0;
    let empty = world.resource::<HierarchyTreeCache>().nodes.is_empty();
    if !dirty && !empty {
        return;
    }

    // Debounce churn-driven rebuilds. `build_entity_tree` is a full-world scan,
    // and this runs exclusively (`&mut World`) on the critical path. The editor
    // constantly spawns/despawns *named* UI chrome (status-bar metrics, reactive
    // rows, tooltips), which trips the dirty flag via `Name`/`ChildOf` change
    // detection even though none of it is in the tree — so without a guard the
    // tree rebuilds every single frame. The panel doesn't need 60 Hz freshness:
    // once a tree exists, rebuild at most ~10x/sec and leave the dirty flag set
    // until a rebuild actually lands (a real scene edit still shows within 100ms).
    let now = world.resource::<Time>().elapsed_secs();
    if !empty && now - *last_build < 0.1 {
        return;
    }
    *last_build = now;

    let nodes = world.resource_scope(|world, mut seq: Mut<HierarchySpawnSeq>| {
        build_entity_tree(world, &mut seq)
    });
    let mut cache = world.resource_mut::<HierarchyTreeCache>();
    cache.nodes = nodes;
    cache.version = cache.version.wrapping_add(1);
    world.resource_mut::<HierarchyDirty>().0 = false;
}
