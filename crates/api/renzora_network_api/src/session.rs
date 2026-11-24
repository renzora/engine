//! Questions a script asks about the session, and the requests it makes.
//!
//! Free functions over `&World` / `&mut World` because that is what a Rust
//! script holds (`ctx.world()`), and because every answer must be right with
//! networking stripped too: a single-player build has no `NetworkStatus`, and
//! there every entity is this process's to simulate.

use bevy::prelude::*;

use crate::components::{NetworkId, NetworkOwner, NetworkTransform, Networked, OwnerKind, TransformAuthority};
use crate::rpc::{NetOutbox, NetRequest};
use crate::status::{NetRole, NetworkStatus};

/// What this process is in the session. [`NetRole::Offline`] without
/// networking.
pub fn role(world: &World) -> NetRole {
    world.get_resource::<NetworkStatus>().map(|s| s.role).unwrap_or_default()
}

/// Whether this process is the server (dedicated or host).
pub fn is_server(world: &World) -> bool {
    role(world).is_server()
}

/// Whether this process is a client of a server elsewhere.
pub fn is_client(world: &World) -> bool {
    role(world) == NetRole::Client
}

/// Whether there is a live session: a client that finished connecting, or a
/// running server.
pub fn is_connected(world: &World) -> bool {
    world.get_resource::<NetworkStatus>().is_some_and(|s| s.is_connected())
}

/// This peer's id, once it has one.
pub fn local_peer_id(world: &World) -> Option<u64> {
    world.get_resource::<NetworkStatus>().and_then(|s| s.client_id)
}

/// Every player in the session, this one included.
pub fn players(world: &World) -> Vec<u64> {
    world.get_resource::<NetworkStatus>().map(|s| s.players.clone()).unwrap_or_default()
}

/// The nearest entity at or above `entity` that is [`Networked`].
///
/// An avatar is a prefab instance: the networked entity is the instance root,
/// and the mesh and the script that moves it are its children. A script asking
/// "is this mine" or "where am I" means the root.
pub fn networked_root(world: &World, entity: Entity) -> Option<Entity> {
    let mut current = Some(entity);
    while let Some(e) = current {
        let er = world.get_entity(e).ok()?;
        if er.contains::<Networked>() {
            return Some(e);
        }
        current = er.get::<ChildOf>().map(|c| c.parent());
    }
    None
}

/// Who owns the networked entity at or above `entity`.
pub fn owner_of(world: &World, entity: Entity) -> Option<OwnerKind> {
    let root = networked_root(world, entity)?;
    Some(world.get::<NetworkOwner>(root).map(|o| o.0).unwrap_or_default())
}

/// The network id of the networked entity at or above `entity`.
pub fn net_id_of(world: &World, entity: Entity) -> Option<u64> {
    let root = networked_root(world, entity)?;
    world.get::<NetworkId>(root).map(|id| id.0)
}

/// The local entity a network id names, if this peer has it.
pub fn entity_by_net_id(world: &mut World, id: u64) -> Option<Entity> {
    let mut q = world.query::<(Entity, &NetworkId)>();
    q.iter(world).find(|(_, n)| n.0 == id).map(|(e, _)| e)
}

/// Whether the entity (or its networked ancestor) belongs to this peer's
/// player.
pub fn is_mine(world: &World, entity: Entity) -> bool {
    match (owner_of(world, entity), local_peer_id(world)) {
        (Some(OwnerKind::Client(owner)), Some(me)) => owner == me,
        _ => false,
    }
}

/// Whether this process should simulate the entity: move it, run its game
/// logic, decide what happens to it.
///
/// True for anything not networked, and for everything when there is no
/// session, so a multiplayer scene still plays single-player. In a session the
/// owner of an owner-moved entity simulates it, and the server simulates the
/// rest. A script that guards its logic with this runs it exactly once across
/// the session instead of once per screen.
pub fn has_authority(world: &World, entity: Entity) -> bool {
    let role = role(world);
    if role == NetRole::Offline {
        return true;
    }
    let Some(root) = networked_root(world, entity) else {
        return true;
    };
    let owner = world.get::<NetworkOwner>(root).map(|o| o.0).unwrap_or_default();
    let owner_moves = world
        .get::<NetworkTransform>(root)
        .is_some_and(|t| t.authority == TransformAuthority::Owner);
    match owner {
        OwnerKind::Client(id) if owner_moves => local_peer_id(world) == Some(id),
        _ => role.is_server(),
    }
}

/// Join a server. An empty address or a zero port takes the project's
/// `[network]` value.
pub fn connect(world: &mut World, address: impl Into<String>, port: u16) {
    world
        .get_resource_or_insert_with(NetOutbox::default)
        .requests
        .push(NetRequest::Connect { address: address.into(), port });
}

/// Leave the session.
pub fn disconnect(world: &mut World) {
    world.get_resource_or_insert_with(NetOutbox::default).requests.push(NetRequest::Disconnect);
}

/// Spawn a networked instance of `prefab` (a project-relative `.bsn`) on the
/// server, owned by `owner`. Every client gets it.
///
/// Server only: a client cannot create shared state, it asks the server to
/// with an RPC. Returns `None` on a client, with nothing spawned.
pub fn spawn_prefab(
    world: &mut World,
    prefab: impl Into<String>,
    transform: Transform,
    owner: OwnerKind,
) -> Option<Entity> {
    if role(world) != NetRole::Offline && !is_server(world) {
        warn!("[network] spawn_prefab on a client does nothing; ask the server with an RPC");
        return None;
    }
    let prefab = prefab.into();
    let name = std::path::Path::new(&prefab)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Networked".into());
    let transform_settings = match owner {
        OwnerKind::Client(_) => NetworkTransform::owner_driven(),
        OwnerKind::Server => NetworkTransform::default(),
    };
    Some(
        world
            .spawn((
                Name::new(name),
                transform,
                Visibility::default(),
                Networked,
                NetworkOwner(owner),
                transform_settings,
                crate::NetworkPrefab(prefab),
            ))
            .id(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::HOST_PEER_ID;

    fn session(role: NetRole, me: Option<u64>) -> World {
        let mut world = World::new();
        world.insert_resource(NetworkStatus { role, client_id: me, ..Default::default() });
        world
    }

    #[test]
    fn everything_is_ours_offline() {
        let mut world = World::new();
        let e = world.spawn((Networked, NetworkOwner(OwnerKind::Server))).id();
        assert!(has_authority(&world, e));
    }

    #[test]
    fn a_child_of_an_avatar_answers_for_the_avatar() {
        let mut world = session(NetRole::Client, Some(42));
        let avatar = world
            .spawn((Networked, NetworkOwner(OwnerKind::Client(42)), NetworkTransform::owner_driven()))
            .id();
        let mesh = world.spawn(ChildOf(avatar)).id();
        assert_eq!(networked_root(&world, mesh), Some(avatar));
        assert!(is_mine(&world, mesh));
        assert!(has_authority(&world, mesh));
    }

    #[test]
    fn the_server_simulates_what_the_owner_does_not_move() {
        let mut world = session(NetRole::Client, Some(42));
        let npc = world.spawn((Networked, NetworkOwner(OwnerKind::Server))).id();
        let other = world
            .spawn((Networked, NetworkOwner(OwnerKind::Client(7)), NetworkTransform::owner_driven()))
            .id();
        assert!(!has_authority(&world, npc));
        assert!(!has_authority(&world, other));

        let mut host = session(NetRole::Host, Some(HOST_PEER_ID));
        let npc = host.spawn((Networked, NetworkOwner(OwnerKind::Server))).id();
        let remote = host
            .spawn((Networked, NetworkOwner(OwnerKind::Client(7)), NetworkTransform::owner_driven()))
            .id();
        assert!(has_authority(&host, npc));
        assert!(!has_authority(&host, remote));
    }

    #[test]
    fn a_client_cannot_spawn_shared_state() {
        let mut world = session(NetRole::Client, Some(42));
        assert!(spawn_prefab(&mut world, "p.bsn", Transform::default(), OwnerKind::Server).is_none());
        let mut server = session(NetRole::DedicatedServer, Some(0));
        let e = spawn_prefab(&mut server, "prefabs/coin.bsn", Transform::default(), OwnerKind::Server).unwrap();
        assert_eq!(server.get::<Name>(e).map(|n| n.as_str()), Some("coin"));
    }
}
