//! Entity-subtree snapshot + faithful delete undo.
//!
//! Distinct from both a scene save and a prefab: this keeps the roots
//! themselves *and* their `ChildOf` links, so a restore puts each entity back
//! under its original parent rather than at the scene root.

use bevy::prelude::*;
use renzora::MeshInstanceData;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::DynamicSceneBuilder;
use std::collections::BTreeSet;

use super::deny::{DenyOptionalSubsystems, DenyUiCameraTargets};
use super::load::deserialize_scene_lossy;

/// Serialize the given root entities **and their descendants** to a BSN string —
/// a faithful, all-components, with-hierarchy snapshot used to undo a delete.
/// Unlike [`super::instances::save_prefab_source`] this keeps the roots
/// themselves and their `ChildOf` links (so restore puts each entity back under
/// its original parent), and stops descending into `MeshInstanceData` subtrees
/// (their runtime gltf children are rebuilt by
/// [`super::meshes::rehydrate_mesh_instances`] on restore). Returns `None` if
/// nothing serializable was captured.
pub fn snapshot_entity_subtrees(world: &mut World, roots: &[Entity]) -> Option<String> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut all: Vec<Entity> = Vec::new();
    let mut seen: BTreeSet<Entity> = BTreeSet::new();
    let mut queue: Vec<Entity> = roots.to_vec();
    while let Some(e) = queue.pop() {
        if !seen.insert(e) {
            continue;
        }
        if world.get_entity(e).is_err() {
            continue;
        }
        all.push(e);
        // Don't descend into gltf-owned runtime subtrees (rehydrated on restore).
        if world.get::<MeshInstanceData>(e).is_some() {
            continue;
        }
        if let Some(kids) = world.get::<Children>(e) {
            queue.extend(kids.iter());
        }
    }
    if all.is_empty() {
        return None;
    }

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_render_3d_materials()
        .deny_terrain_material()
        .deny_ui_camera_targets()
        .deny_component::<ViewVisibility>()
        // Children are rebuilt from the ChildOf links we DO keep.
        .deny_component::<Children>()
        .deny_component::<GlobalTransform>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        // Runtime mirror of the camera's projection, rebuilt every frame.
        .deny_component::<crate::camera_script::CameraReadState>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_animation_state()
        .deny_physics_components()
        .extract_entities(all.into_iter())
        .build();

    // Drop editor-only / non-serializable components (mirror save_prefab_source),
    // but KEEP ChildOf so the entity restores under its original parent.
    for entity in &mut scene.entities {
        entity.components.retain(|component| {
            let type_name = component.reflect_type_path();
            if type_name.starts_with("bevy_mod_outline::") {
                return false;
            }
            if type_name.starts_with("avian3d::") || type_name.starts_with("avian2d::") {
                return false;
            }
            // Gaussian-splat runtime components are resolved on load from the
            // serializable renzora::GaussianSplat — same filter as `save_scene`.
            if type_name.starts_with("bevy_gaussian_splatting::") {
                return false;
            }
            let registry = type_registry.read();
            let serializer = bevy::reflect::serde::TypedReflectSerializer::new(
                component.as_partial_reflect(),
                &registry,
            );
            ron::ser::to_string(&serializer).is_ok()
        });
    }

    let registry = type_registry.read();
    BsnSerializer.serialize(&scene, &registry).ok()
}

/// Respawn entities from a [`snapshot_entity_subtrees`] string, returning the
/// old→new entity id map (so a delete-undo command can find the restored roots).
pub fn spawn_entities_from_snapshot(
    world: &mut World,
    ron: &str,
) -> bevy::ecs::entity::EntityHashMap<Entity> {
    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    if ron.trim().is_empty() {
        return entity_map;
    }
    let (scene, _skipped) = match deserialize_scene_lossy(world, ron) {
        Ok(pair) => pair,
        Err(e) => {
            error!("[undo] failed to deserialize entity snapshot: {}", e);
            return entity_map;
        }
    };

    // A snapshot keeps each entity's `ChildOf`, but the parent of a deleted
    // *root* lives outside the snapshot, so its id is absent from `entity_map`.
    // `write_to_world` remaps every entity reference through that map, and
    // `SceneEntityMapper` turns an absent id into a freshly reserved DEAD id —
    // the relationship hook then drops the `ChildOf`, so the entity restores at
    // the scene root instead of under its parent (issue #75). Seed an identity
    // entry for each external parent that is still live so the link survives the
    // remap. Only the parents the snapshot actually references are seeded, read
    // straight off its own `ChildOf` links — not the whole world. Snapshot-
    // internal entities are deliberately left unseeded: they still receive fresh
    // ids, so the links between them follow the remap.
    let snapshot_ids: bevy::ecs::entity::EntityHashSet =
        scene.entities.iter().map(|e| e.entity).collect();
    for dynamic_entity in &scene.entities {
        for component in &dynamic_entity.components {
            let Some(child_of) = ChildOf::from_reflect(component.as_partial_reflect()) else {
                continue;
            };
            let parent = child_of.parent();
            if !snapshot_ids.contains(&parent) && world.get_entity(parent).is_ok() {
                entity_map.insert(parent, parent);
            }
        }
    }

    if let Err(e) = scene.write_to_world(world, &mut entity_map) {
        error!("[undo] failed to restore entity snapshot: {}", e);
        return entity_map;
    }

    // Narrow the returned map to the snapshot's own old->new pairs; the identity
    // seeds above are scaffolding for the remap, not results. The sole caller
    // (`DeleteEntitiesCmd::undo`) only looks up restored roots.
    let restored: bevy::ecs::entity::EntityHashMap<Entity> = scene
        .entities
        .iter()
        .filter_map(|e| entity_map.get(&e.entity).map(|new| (e.entity, *new)))
        .collect();

    // Re-insert ChildOf so hierarchy hooks fire (same as load_scene_from_string).
    let children_with_parents: Vec<(Entity, Entity)> = restored
        .values()
        .filter_map(|&entity| {
            world
                .get_entity(entity)
                .ok()?
                .get::<ChildOf>()
                .map(|c| (entity, c.parent()))
        })
        .collect();
    for (child, parent) in children_with_parents {
        world.entity_mut(child).remove::<ChildOf>();
        world.entity_mut(child).insert(ChildOf(parent));
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deleting a *child* and undoing must put it back under the same parent.
    /// The parent is outside the snapshot, so its id is absent from the
    /// entity map that `write_to_world` remaps through.
    #[test]
    fn snapshot_restore_reattaches_root_to_its_original_parent() {
        let atr = bevy::ecs::reflect::AppTypeRegistry::default();
        {
            let mut reg = atr.write();
            reg.register::<Name>();
            reg.register::<ChildOf>();
        }

        let mut world = World::new();
        world.insert_resource(atr.clone());
        let parent = world.spawn(Name::new("Parent")).id();
        let child = world.spawn((Name::new("Child"), ChildOf(parent))).id();

        let snapshot = snapshot_entity_subtrees(&mut world, &[child]).expect("snapshot");
        world.entity_mut(child).despawn();

        let map = spawn_entities_from_snapshot(&mut world, &snapshot);
        let restored = *map.get(&child).expect("child restored");

        assert_eq!(
            world.get::<ChildOf>(restored).map(|c| c.parent()),
            Some(parent),
            "restored entity must point at the original live parent"
        );
        assert!(
            world
                .get::<Children>(parent)
                .is_some_and(|c| c.contains(&restored)),
            "parent must list the restored entity as a child"
        );
    }

    /// The identity seeding that fixes the external-parent link must not leak
    /// into the snapshot's own entities: those still need fresh ids, and the
    /// links *between* them must follow the remap, not the stale originals.
    #[test]
    fn snapshot_restore_remaps_internal_links_to_fresh_ids() {
        let atr = bevy::ecs::reflect::AppTypeRegistry::default();
        {
            let mut reg = atr.write();
            reg.register::<Name>();
            reg.register::<ChildOf>();
        }

        let mut world = World::new();
        world.insert_resource(atr.clone());
        let grandparent = world.spawn(Name::new("Grandparent")).id();
        let root = world.spawn((Name::new("Root"), ChildOf(grandparent))).id();
        let leaf = world.spawn((Name::new("Leaf"), ChildOf(root))).id();

        let snapshot = snapshot_entity_subtrees(&mut world, &[root]).expect("snapshot");
        world.entity_mut(root).despawn();
        assert!(world.get_entity(leaf).is_err(), "despawn takes the subtree");

        let map = spawn_entities_from_snapshot(&mut world, &snapshot);
        let new_root = *map.get(&root).expect("root restored");
        let new_leaf = *map.get(&leaf).expect("leaf restored");

        assert_eq!(
            world.get::<ChildOf>(new_root).map(|c| c.parent()),
            Some(grandparent),
            "root reattaches to the untouched grandparent"
        );
        assert_eq!(
            world.get::<ChildOf>(new_leaf).map(|c| c.parent()),
            Some(new_root),
            "leaf follows the remap to the root's NEW id, not the stale one"
        );
        assert_eq!(
            map.len(),
            2,
            "map reports only the restored entities, not the identity seeds"
        );
    }
}
