//! Two worlds, one entity: the replication primitive itself.
//!
//! This is the claim the whole feature rests on — that a snapshot taken in one
//! editor can be applied to the *matching* entity in another, rather than
//! spawning a second copy of it beside the original. Everything else (change
//! detection, ids, the link) only decides *when* to do this; if the apply is
//! wrong then a session slowly fills the scene with duplicates and there is no
//! layer above that can notice.

use bevy::prelude::*;
use renzora_engine::scene_io::{
    apply_entity_snapshot, snapshot_entities, Descend, ExternalParents,
};

/// A world with just enough registered for the scene serializer to work.
fn world() -> World {
    let mut world = World::new();
    let registry = AppTypeRegistry::default();
    {
        let mut types = registry.write();
        types.register::<Name>();
        types.register::<Transform>();
        types.register::<ChildOf>();
        types.register::<Visibility>();
    }
    world.insert_resource(registry);
    world
}

fn named(world: &World, name: &str) -> Vec<Entity> {
    world
        .iter_entities()
        .filter(|e| e.get::<Name>().is_some_and(|n| n.as_str() == name))
        .map(|e| e.id())
        .collect()
}

/// A seeded apply lands on the entity that is already there.
#[test]
fn upsert_patches_the_existing_entity() {
    let mut sender = world();
    let remote = sender
        .spawn((Name::new("Cube"), Transform::from_xyz(5.0, 0.0, 0.0)))
        .id();
    let bsn = snapshot_entities(&mut sender, &[remote], Descend::ExactSet)
        .expect("the entity should serialize");

    // The receiver already has its own copy, somewhere else entirely.
    let mut receiver = world();
    let local = receiver
        .spawn((Name::new("Cube"), Transform::from_xyz(0.0, 0.0, 0.0)))
        .id();

    let mut seed = bevy::ecs::entity::EntityHashMap::default();
    seed.insert(remote, local);
    apply_entity_snapshot(&mut receiver, &bsn, &seed, ExternalParents::MappedOnly);

    assert_eq!(named(&receiver, "Cube").len(), 1, "the apply duplicated the entity");
    assert_eq!(
        receiver.get::<Transform>(local).map(|t| t.translation.x),
        Some(5.0),
        "the existing entity was not updated in place"
    );
}

/// Without a seed the same snapshot spawns — which is how an entity a peer
/// created for the first time arrives.
#[test]
fn unseeded_upsert_spawns() {
    let mut sender = world();
    let remote = sender.spawn((Name::new("Lamp"), Transform::from_xyz(2.0, 3.0, 4.0))).id();
    let bsn = snapshot_entities(&mut sender, &[remote], Descend::ExactSet).expect("serialize");

    let mut receiver = world();
    assert!(named(&receiver, "Lamp").is_empty());

    let applied = apply_entity_snapshot(
        &mut receiver,
        &bsn,
        &bevy::ecs::entity::EntityHashMap::default(),
        ExternalParents::MappedOnly,
    );

    let spawned = named(&receiver, "Lamp");
    assert_eq!(spawned.len(), 1, "expected exactly one new entity");
    assert_eq!(
        receiver.get::<Transform>(spawned[0]).map(|t| t.translation),
        Some(Vec3::new(2.0, 3.0, 4.0))
    );
    // The returned map is what binds the sender's id to the local entity.
    assert_eq!(applied.get(&remote), Some(&spawned[0]));
}

/// Applying the same snapshot repeatedly must converge, not accumulate. A live
/// session does exactly this — the same entity is re-sent every time it moves.
#[test]
fn repeated_applies_do_not_accumulate() {
    let mut sender = world();
    let remote = sender.spawn((Name::new("Rock"), Transform::from_xyz(1.0, 0.0, 0.0))).id();
    let bsn = snapshot_entities(&mut sender, &[remote], Descend::ExactSet).expect("serialize");

    let mut receiver = world();
    let mut seed = bevy::ecs::entity::EntityHashMap::default();
    for _ in 0..10 {
        let applied =
            apply_entity_snapshot(&mut receiver, &bsn, &seed, ExternalParents::MappedOnly);
        // Bind after the first apply, exactly as the session does.
        if let Some(&local) = applied.get(&remote) {
            seed.insert(remote, local);
        }
    }
    assert_eq!(named(&receiver, "Rock").len(), 1);
}

/// `ExactSet` sends only what changed. Descending would re-send every child of a
/// parent that merely moved, which for a tilemap layer is thousands of entities
/// several times a second.
#[test]
fn exact_set_does_not_drag_in_children() {
    let mut sender = world();
    let parent = sender.spawn((Name::new("Parent"), Transform::default())).id();
    sender.spawn((Name::new("Child"), Transform::default(), ChildOf(parent)));

    let exact = snapshot_entities(&mut sender, &[parent], Descend::ExactSet).expect("serialize");
    assert!(!exact.contains("Child"), "ExactSet pulled in a descendant");

    let subtree = snapshot_entities(&mut sender, &[parent], Descend::Subtree).expect("serialize");
    assert!(subtree.contains("Child"), "Subtree should include descendants");
}

/// A parent link survives when the parent is named in the seed, and is dropped
/// rather than misapplied when it is not — the sender's entity ids mean nothing
/// in the receiver's world, so an unmapped one must never be trusted.
#[test]
fn parent_links_follow_the_seed() {
    let mut sender = world();
    let parent = sender.spawn((Name::new("Rig"), Transform::default())).id();
    let child = sender
        .spawn((Name::new("Bone"), Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(parent)))
        .id();
    let bsn = snapshot_entities(&mut sender, &[child], Descend::ExactSet).expect("serialize");

    // Receiver has its own Rig; the seed is what says the two are the same rig.
    let mut receiver = world();
    let local_parent = receiver.spawn((Name::new("Rig"), Transform::default())).id();
    let mut seed = bevy::ecs::entity::EntityHashMap::default();
    seed.insert(parent, local_parent);

    let applied = apply_entity_snapshot(&mut receiver, &bsn, &seed, ExternalParents::MappedOnly);
    let local_child = *applied.get(&child).expect("the child should have been spawned");
    assert_eq!(
        receiver.get::<ChildOf>(local_child).map(|c| c.parent()),
        Some(local_parent),
        "the child did not land under the mapped parent"
    );
}
