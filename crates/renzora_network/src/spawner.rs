//! Player avatars: one per peer, from the scene's `NetworkPlayerSpawner`, and
//! the prefab instancing that gives every networked entity its looks.

use bevy::prelude::*;
use renzora_network_api::{
    spawn_prefab, NetworkOwner, NetworkPlayer, NetworkPlayerSpawner, NetworkPrefab, OwnerKind,
};

/// Peers owed an avatar. Joins are queued rather than spawned on the spot
/// because a player can join before the server's scene (and so its spawner)
/// has loaded; the queue waits for it.
#[derive(Resource, Default)]
pub struct PendingAvatars(pub Vec<u64>);

/// Spawn the avatars owed, once a spawner exists. Server only, by
/// construction: only the server queues anything.
pub fn spawn_avatars(world: &mut World) {
    if world.resource::<PendingAvatars>().0.is_empty() {
        return;
    }
    let mut spawners = world.query::<(&NetworkPlayerSpawner, Option<&GlobalTransform>, Option<&Transform>)>();
    let Some((prefab, radius, origin)) = spawners.iter(world).next().map(|(s, g, t)| {
        let origin = g.map(|g| g.translation()).or(t.map(|t| t.translation)).unwrap_or_default();
        (s.prefab.clone(), s.spawn_radius, origin)
    }) else {
        return;
    };
    if prefab.trim().is_empty() {
        warn!("[network] NetworkPlayerSpawner has no prefab set, so no avatars are spawned");
        world.resource_mut::<PendingAvatars>().0.clear();
        return;
    }

    let pending = std::mem::take(&mut world.resource_mut::<PendingAvatars>().0);
    let mut existing = world.query_filtered::<&NetworkOwner, With<NetworkPlayer>>();
    let mut have: Vec<OwnerKind> = existing.iter(world).map(|o| o.0).collect();
    for peer in pending {
        let owner = OwnerKind::Client(peer);
        if have.contains(&owner) {
            continue;
        }
        // Spread around the ring by the golden angle, so any number of
        // players are evenly apart with no table of slots to keep.
        let angle = have.len() as f32 * 2.399_963;
        let at = origin + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
        if let Some(e) = spawn_prefab(world, prefab.clone(), Transform::from_translation(at), owner) {
            world.entity_mut(e).insert((NetworkPlayer, Name::new(format!("Player {peer}"))));
            info!("[network] Spawned avatar for player {peer}");
        }
        have.push(owner);
    }
}

/// Give a networked entity its looks: a `NetworkPrefab` becomes a scene
/// instance, expanded by the engine on every peer from its own copy of the
/// project.
pub fn instance_prefabs(
    mut commands: Commands,
    q: Query<(Entity, &NetworkPrefab), Changed<NetworkPrefab>>,
    instances: Query<&renzora_scene_api::SceneInstance>,
) {
    for (e, prefab) in &q {
        if prefab.0.is_empty() || instances.get(e).is_ok_and(|i| i.source == prefab.0) {
            continue;
        }
        commands.entity(e).insert(renzora_scene_api::SceneInstance {
            source: prefab.0.clone(),
            ..Default::default()
        });
    }
}
