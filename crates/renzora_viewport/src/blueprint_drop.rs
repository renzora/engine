//! Drag-and-drop blueprint attachment — when the user drags a `.blueprint` from
//! the asset browser onto an entity in the viewport, load that graph and attach
//! it as a `BlueprintGraph` component so the interpreter runs it.
//!
//! This is the blueprint analogue of `material_drop`: it raycasts for the mesh
//! under the pointer, walks up to the nearest *named* ancestor (so the graph
//! lands on the logical entity, not a sub-mesh of an imported model), and inserts
//! the deserialized graph. A `.blueprint` file is a JSON-serialized
//! `BlueprintGraph` (the same component the scene serializes).

use std::path::PathBuf;

use bevy::prelude::*;

use renzora_blueprint::graph::BlueprintGraph;
use renzora_editor_framework::EditorSelection;

use crate::material_drop::pick_mesh_under_pointer;

pub(crate) const BLUEPRINT_EXTENSIONS: &[&str] = &["blueprint", "bp"];

/// Commit a `.blueprint` drop — pick the entity under `screen_pos`, load the
/// graph from `path`, and insert it as a `BlueprintGraph`. Shared by the native
/// bevy_ui drop (`native_drop::commit_viewport_drop`).
pub(crate) fn commit_blueprint_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    path: PathBuf,
) {
    let Some(picked) = pick_mesh_under_pointer(world, screen_pos, vp_rect) else {
        info!("[blueprint_drop] No entity under pointer — ignoring drop");
        return;
    };
    let target = named_ancestor(world, picked).unwrap_or(picked);

    let graph = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str::<BlueprintGraph>(&json).ok())
    {
        Some(g) => g,
        None => {
            warn!("[blueprint_drop] Failed to read/parse {:?}", path);
            return;
        }
    };

    if world.get_entity(target).is_err() {
        return;
    }
    world.entity_mut(target).insert(graph);
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(target));
    }
    info!("[blueprint_drop] Attached {:?} to entity {:?}", path, target);
}

/// Walk up the parent chain to the nearest ancestor that has a `Name` (the
/// logical entity), so dropping on a model's sub-mesh attaches to the model root.
fn named_ancestor(world: &World, entity: Entity) -> Option<Entity> {
    let mut current = entity;
    loop {
        if world.get::<Name>(current).is_some() {
            return Some(current);
        }
        match world.get::<ChildOf>(current) {
            Some(parent) => current = parent.parent(),
            None => return None,
        }
    }
}
