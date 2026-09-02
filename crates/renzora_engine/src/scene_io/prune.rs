//! Load-time repair passes over a deserialized scene.
//!
//! Both of these exist to undo damage an earlier, less careful save did. A
//! scene is not trusted to be well-formed: an over-eager auto-save could bake
//! the editor's own UI tree into it, and those nodes paint full-window over
//! everything on reload. The passes run before the scene reaches the World, so
//! the bad entities are never spawned rather than spawned-then-despawned.

use bevy::prelude::*;
use renzora_bsn::{DynamicEntity, DynamicScene};

/// The `ChildOf` parent recorded on a serialized scene entity, if any. Read
/// straight out of the reflected components — the scene isn't in the World yet,
/// so we can't query it.
fn scene_entity_parent(dyn_ent: &DynamicEntity) -> Option<Entity> {
    for comp in &dyn_ent.components {
        let is_child_of = comp
            .get_represented_type_info()
            .map(|ti| ti.type_path() == <ChildOf as bevy::reflect::TypePath>::type_path())
            .unwrap_or(false);
        if !is_child_of {
            continue;
        }
        if let bevy::reflect::ReflectRef::TupleStruct(ts) = comp.reflect_ref() {
            if let Some(parent) = ts.field(0).and_then(|f| f.try_downcast_ref::<Entity>()) {
                return Some(*parent);
            }
        }
    }
    None
}

/// Drop entities whose `ChildOf` ancestor chain leads to a parent that ISN'T in
/// the scene. Such an entity is an orphan of a root that was excluded at save
/// time — almost always leaked editor-chrome widgets: the `HideInHierarchy`
/// shell root is correctly filtered out of saves, but older scenes baked in its
/// named children (dock tabs, glyph icons, inspector rows). On load those would
/// reparent to the window root and paint full-window over the editor (blank).
///
/// Cascades for free: a child of a pruned entity is pruned too, because its own
/// chain still climbs to the same missing root. A well-formed scene has complete
/// hierarchies, so nothing is dropped. Returns how many were pruned.
pub(crate) fn prune_orphaned_entities(scene: &mut DynamicScene) -> usize {
    use std::collections::{HashMap, HashSet};
    let ids: HashSet<Entity> = scene.entities.iter().map(|e| e.entity).collect();
    if ids.is_empty() {
        return 0;
    }
    let parent_of: HashMap<Entity, Entity> = scene
        .entities
        .iter()
        .filter_map(|e| scene_entity_parent(e).map(|p| (e.entity, p)))
        .collect();

    let orphaned = |start: Entity| -> bool {
        let mut cur = start;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(cur) {
                return false; // cycle — keep rather than loop forever
            }
            match parent_of.get(&cur) {
                None => return false,                   // a root → valid
                Some(p) if ids.contains(p) => cur = *p, // climb toward the root
                Some(_) => return true,                 // parent absent → orphan
            }
        }
    };

    let before = scene.entities.len();
    // Restrict to UI entities: leaked chrome is always `bevy_ui` nodes, so this
    // can never drop legit 3D scene data even if some non-UI entity were
    // orphaned for an unrelated reason.
    scene.entities.retain(|e| !(orphaned(e.entity) && scene_entity_is_ui(e)));
    before - scene.entities.len()
}

/// Whether a serialized scene entity is a `bevy_ui` node (carries `Node`).
fn scene_entity_is_ui(dyn_ent: &DynamicEntity) -> bool {
    dyn_ent.components.iter().any(|c| {
        c.get_represented_type_info()
            .map(|ti| ti.type_path() == "bevy_ui::ui_node::Node")
            .unwrap_or(false)
    })
}

/// Whether a serialized scene entity is a game-UI `UiCanvas` root. Legitimate
/// game UI lives under one of these; matched by reflected type-path so this crate
/// needn't depend on `renzora_ember`.
fn scene_entity_is_canvas(dyn_ent: &DynamicEntity) -> bool {
    dyn_ent.components.iter().any(|c| {
        c.get_represented_type_info()
            .map(|ti| ti.type_path() == "renzora_ember::game_ui::components::canvas::UiCanvas")
            .unwrap_or(false)
    })
}

/// Drop leaked editor UI: any `bevy_ui` node with no `UiCanvas` self-or-ancestor.
///
/// The only legitimate UI in a scene is game UI, which always sits under a
/// `UiCanvas` root (the serializable source of truth, rebuilt on load); 3D
/// content carries no `Node`. So a `Node` entity outside every canvas is editor
/// chrome an over-eager save baked in — classically auto-save firing while an
/// overlay (e.g. Settings) was open, serializing its whole node tree. Unlike
/// [`prune_orphaned_entities`] (which only catches nodes whose parent is missing)
/// this also removes *connected* chrome trees that kept an intact root, so it
/// self-heals scenes already polluted before the save-side guard existed.
pub(crate) fn prune_leaked_ui(scene: &mut DynamicScene) -> usize {
    use std::collections::{HashMap, HashSet};
    let ids: HashSet<Entity> = scene.entities.iter().map(|e| e.entity).collect();
    if ids.is_empty() {
        return 0;
    }
    let parent_of: HashMap<Entity, Entity> = scene
        .entities
        .iter()
        .filter_map(|e| scene_entity_parent(e).map(|p| (e.entity, p)))
        .collect();
    let canvases: HashSet<Entity> = scene
        .entities
        .iter()
        .filter(|e| scene_entity_is_canvas(e))
        .map(|e| e.entity)
        .collect();

    // Whether `start` or any in-scene ancestor is a `UiCanvas`.
    let under_canvas = |start: Entity| -> bool {
        let mut cur = start;
        let mut seen = HashSet::new();
        loop {
            if canvases.contains(&cur) {
                return true;
            }
            if !seen.insert(cur) {
                return false; // cycle guard
            }
            match parent_of.get(&cur) {
                Some(p) if ids.contains(p) => cur = *p,
                _ => return false,
            }
        }
    };

    let before = scene.entities.len();
    scene
        .entities
        .retain(|e| !scene_entity_is_ui(e) || under_canvas(e.entity));
    before - scene.entities.len()
}
