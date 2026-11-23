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

use bevy::ecs::archetype::ArchetypeId;
use bevy::ecs::entity::Entities;
use std::collections::HashMap;

use crate::state::{build_entity_tree, EntityNode, HierarchyCandidate, HierarchySpawnSeq};

/// Cached entity tree, produced by `update_hierarchy_cache`.
#[derive(Resource, Default)]
pub struct HierarchyTreeCache {
    pub nodes: Vec<EntityNode>,
    /// Monotonic counter; consumers can compare against a stored value to
    /// detect rebuilds without diffing the tree.
    pub version: u64,
    /// Every entity in the tree, with the archetype it had when the tree was
    /// built. A removal names an entity that may already be gone, so it is
    /// checked against this rather than filtered. The archetype catches a
    /// component added or removed: a row's icon comes from whichever registered
    /// component it carries (a light, a camera, a mesh), and there are too many
    /// of those to watch one by one.
    pub(crate) members: HashMap<Entity, ArchetypeId>,
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
///
/// Only changes that could show in the tree count. The editor spawns,
/// renames and despawns its own named UI every frame (status-bar readouts,
/// reactive rows, tooltips), and when any `Name` or `ChildOf` anywhere counted,
/// the tree was rebuilt ten times a second with nothing in the scene changing.
/// Changes are filtered to the tree's own candidates, and removals, whose
/// entity may be gone, to the entities the tree last held.
#[allow(clippy::too_many_arguments)]
pub fn mark_hierarchy_dirty(
    mut dirty: ResMut<HierarchyDirty>,
    filter: Option<Res<HierarchyFilter>>,
    cache: Res<HierarchyTreeCache>,
    changed_name: Query<(), (Or<(Added<Name>, Changed<Name>)>, HierarchyCandidate)>,
    changed_child_of: Query<(), (Changed<ChildOf>, HierarchyCandidate)>,
    changed_visibility: Query<(), (Changed<Visibility>, HierarchyCandidate)>,
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
    // Asset badges (script/material) ride on these components, so
    // their add/change/remove must rebuild the tree too (grouped into one param
    // to stay under Bevy's per-system param-count cap).
    mut badges: AssetBadgeChanges,
    entities: &Entities,
) {
    // Read every removal stream each frame, dirty or not: an unread stream
    // keeps its events, and the next frame would act on stale ones.
    let in_tree = |hit: bool, e: Entity| hit | cache.members.contains_key(&e);
    let removed = removed_name.read().fold(false, in_tree)
        | removed_child_of.read().fold(false, in_tree)
        | removed_hide.read().fold(false, in_tree);
    let identity_dirty = identity.dirty();
    let badges_dirty = badges.dirty();
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
        || removed
        || identity_dirty
        || badges_dirty
    {
        dirty.0 = true;
        return;
    }

    // A component added to or removed from a row's entity (adding a light
    // changes its icon, undoing that changes it back) moves it to another
    // archetype. Before the tree stopped rebuilding on unrelated UI churn, that
    // churn happened to repaint such a row within a tenth of a second.
    let moved = cache.members.iter().any(|(&e, &archetype)| {
        entities.get_spawned(e).map(|l| l.archetype_id).ok() != Some(archetype)
    });
    if moved {
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
    changed_material: Query<'w, 's, (), Changed<renzora_material_api::MaterialRef>>,
    removed_script: RemovedComponents<'w, 's, renzora_scripting::ScriptComponent>,
    removed_material: RemovedComponents<'w, 's, renzora_material_api::MaterialRef>,
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
    if !world.resource::<HierarchyDirty>().0 {
        return;
    }
    // "Never built" is the version, not an empty tree: an empty scene builds an
    // empty tree, and treating that as unbuilt rebuilt it every frame with no
    // debounce. `HierarchyDirty` starts set, so the first build still happens.
    let never_built = world.resource::<HierarchyTreeCache>().version == 0;

    // Debounce churn-driven rebuilds. `build_entity_tree` is a full-world scan,
    // and this runs exclusively (`&mut World`) on the critical path. The editor
    // constantly spawns/despawns *named* UI chrome (status-bar metrics, reactive
    // rows, tooltips), which trips the dirty flag via `Name`/`ChildOf` change
    // detection even though none of it is in the tree — so without a guard the
    // tree rebuilds every single frame. The panel doesn't need 60 Hz freshness:
    // once a tree exists, rebuild at most ~10x/sec and leave the dirty flag set
    // until a rebuild actually lands (a real scene edit still shows within 100ms).
    let now = world.resource::<Time>().elapsed_secs();
    if !never_built && now - *last_build < 0.1 {
        return;
    }
    *last_build = now;

    let nodes = world.resource_scope(|world, mut seq: Mut<HierarchySpawnSeq>| {
        build_entity_tree(world, &mut seq)
    });
    stamp_root_order(world, &nodes);
    let mut members = HashMap::new();
    let mut stack: Vec<&EntityNode> = nodes.iter().collect();
    while let Some(node) = stack.pop() {
        if let Ok(location) = world.entities().get_spawned(node.entity) {
            members.insert(node.entity, location.archetype_id);
        }
        stack.extend(node.children.iter());
    }
    let mut cache = world.resource_mut::<HierarchyTreeCache>();
    cache.nodes = nodes;
    cache.members = members;
    cache.version = cache.version.wrapping_add(1);
    world.resource_mut::<HierarchyDirty>().0 = false;
}

/// Give every scene root that has no [`HierarchyOrder`] the next one after
/// those that do, in the order the tree shows them, so that order is what the
/// scene saves and what it reopens in. See [`HierarchyOrder`] for why the
/// fallback order could not be trusted across a load.
///
/// Only true roots: an entity shown at the top because its real parent is
/// hidden keeps whatever its parent's order gives it.
fn stamp_root_order(world: &mut World, nodes: &[EntityNode]) {
    let is_root = |world: &World, e: Entity| world.get::<ChildOf>(e).is_none();
    let missing: Vec<Entity> = nodes
        .iter()
        .map(|n| n.entity)
        .filter(|&e| is_root(world, e) && world.get::<HierarchyOrder>(e).is_none())
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut next = nodes
        .iter()
        .filter_map(|n| world.get::<HierarchyOrder>(n.entity))
        .map(|h| h.0.saturating_add(1))
        .max()
        .unwrap_or(0);
    for e in missing {
        world.entity_mut(e).insert(HierarchyOrder(next));
        next = next.saturating_add(1);
    }
}
