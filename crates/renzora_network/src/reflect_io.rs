//! Components to bytes and back, by type path, through reflection.
//!
//! Reflection is what lets a game replicate its own `Health` without the
//! network crate knowing the type: anything registered in the
//! `ReplicationRegistry` and reflected as a component can be read off an
//! entity on the server and written onto one on a client.

use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::reflect::TypeRegistry;
use serde::de::DeserializeSeed;

use crate::protocol::WireComponent;

/// Read every component in `paths` that `entity` has, serialized.
///
/// A path that is not registered for reflection, or not as a component, is
/// skipped: it is a game's mistake, reported once by the caller, not a reason
/// to stop replicating everything else.
pub fn read_components(world: &World, registry: &TypeRegistry, entity: Entity, paths: &[String]) -> Vec<WireComponent> {
    let Ok(er) = world.get_entity(entity) else { return Vec::new() };
    let mut out = Vec::new();
    for path in paths {
        let Some(registration) = registry.get_with_type_path(path) else { continue };
        let Some(rc) = registration.data::<ReflectComponent>() else { continue };
        let Some(value) = rc.reflect(er) else { continue };
        let serializer = TypedReflectSerializer::new(value.as_partial_reflect(), registry);
        match serde_json::to_vec(&serializer) {
            Ok(bytes) => out.push((path.clone(), bytes)),
            Err(e) => warn_once(path, &format!("cannot be serialized for replication: {e}")),
        }
    }
    out
}

/// Write a received component onto `entity`, inserting it or replacing it.
pub fn write_component(world: &mut World, registry: &TypeRegistry, entity: Entity, path: &str, bytes: &[u8]) {
    let Some(registration) = registry.get_with_type_path(path) else {
        warn_once(path, "is not registered on this peer, so it cannot be applied");
        return;
    };
    let Some(rc) = registration.data::<ReflectComponent>() else { return };
    let seed = TypedReflectDeserializer::new(registration, registry);
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = match seed.deserialize(&mut de) {
        Ok(v) => v,
        Err(e) => {
            warn_once(path, &format!("arrived in a shape this peer cannot read: {e}"));
            return;
        }
    };
    let Ok(mut em) = world.get_entity_mut(entity) else { return };
    // `insert` replaces an existing value as well, through FromReflect or the
    // type's default, which is what a changed component needs.
    rc.insert(&mut em, value.as_ref(), registry);
}

/// Remove a component by type path.
pub fn remove_component(world: &mut World, registry: &TypeRegistry, entity: Entity, path: &str) {
    let Some(rc) = registry.get_with_type_path(path).and_then(|r| r.data::<ReflectComponent>()) else {
        return;
    };
    if let Ok(mut em) = world.get_entity_mut(entity) {
        rc.remove(&mut em);
    }
}

/// Log a per-type problem the first time only. Replication runs every tick,
/// and a type that cannot be sent cannot be sent on any of them.
fn warn_once(path: &str, problem: &str) {
    use std::collections::HashSet;
    use std::sync::Mutex;
    static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    let Ok(mut seen) = SEEN.lock() else { return };
    if seen.get_or_insert_with(HashSet::new).insert(path.to_string()) {
        warn!("[network] `{path}` {problem}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_network_api::{NetworkOwner, OwnerKind};

    fn world() -> World {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Name>();
            r.register::<NetworkOwner>();
            r.register::<OwnerKind>();
        }
        world.insert_resource(registry);
        world
    }

    #[test]
    fn a_component_crosses_from_one_world_to_another() {
        let mut server = world();
        let e = server.spawn((Name::new("Avatar"), NetworkOwner(OwnerKind::Client(5)))).id();
        let registry = server.resource::<AppTypeRegistry>().clone();
        let paths = vec![
            <Name as TypePath>::type_path().to_string(),
            <NetworkOwner as TypePath>::type_path().to_string(),
            "not::a::Type".to_string(),
        ];
        let wire = read_components(&server, &registry.read(), e, &paths);
        assert_eq!(wire.len(), 2);

        let mut client = world();
        let c = client.spawn_empty().id();
        let registry = client.resource::<AppTypeRegistry>().clone();
        for (path, bytes) in &wire {
            write_component(&mut client, &registry.read(), c, path, bytes);
        }
        assert_eq!(client.get::<Name>(c).map(|n| n.as_str()), Some("Avatar"));
        assert_eq!(client.get::<NetworkOwner>(c).map(|o| o.0), Some(OwnerKind::Client(5)));

        remove_component(&mut client, &registry.read(), c, <Name as TypePath>::type_path());
        assert!(client.get::<Name>(c).is_none());
    }
}
