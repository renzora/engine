//! The components a scene or a script puts on an entity to make it multiplayer.
//!
//! Every type keeps the `renzora_network::components` type path it had before
//! it moved here. Saved scenes name components by type path, and the export
//! capability detector recognises a multiplayer game by finding that prefix in
//! a scene, so moving the definition must not move the name.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Replicate this entity from the server to every client.
///
/// The server gives it a [`NetworkId`], sends it to every client that joins,
/// streams its `Transform`, and sends any registered component that changes
/// (see [`crate::ReplicationRegistry`]). A client that already has the same
/// entity in its copy of the scene (matched by `SceneEntityId`) adopts it
/// rather than spawning a second one, so a door placed in the editor is one
/// door on every screen.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct Networked;

/// The id a replicated entity has on every peer.
///
/// Assigned by the server. An `Entity` is an index into one world and means
/// nothing in another, so this is what every replication message names. Never
/// saved: it belongs to one session.
#[derive(Component, Clone, Copy, Debug, Reflect, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[reflect(Component, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkId(pub u64);

/// Who owns a networked entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub enum OwnerKind {
    /// The server simulates it (NPCs, doors, pickups). The default for
    /// anything placed in a scene.
    #[default]
    Server,
    /// A connected player owns it, by peer id. The server's player avatars
    /// are this.
    Client(u64),
}

/// Which peer owns this networked entity.
///
/// Ownership decides two things: who may move it when its
/// [`NetworkTransform::authority`] is [`TransformAuthority::Owner`], and which
/// entities are despawned when a player leaves.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkOwner(pub OwnerKind);

/// Marks a player's avatar: one per connected peer, spawned by the server from
/// a [`NetworkPlayerSpawner`] and despawned when that peer leaves.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkPlayer;

/// Who decides where a networked entity is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub enum TransformAuthority {
    /// The server moves it and every client follows. Right for anything the
    /// game rules own, and the only choice that a cheating client cannot bend.
    #[default]
    Server,
    /// The owning client moves it and the server passes the result on. A
    /// player's own avatar then responds with no round trip, which is what
    /// makes movement feel local. The trade is trust: the server takes the
    /// owner's word for where it is.
    Owner,
}

/// How a [`Networked`] entity's `Transform` replicates.
///
/// Optional: without it an entity replicates translation and rotation,
/// server-authoritative and interpolated.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkTransform {
    /// Smooth between received positions on peers that do not move it. Off
    /// snaps to each one, which is jittery but right for something that only
    /// ever teleports.
    pub interpolate: bool,
    /// Send rotation. Translation always goes.
    pub sync_rotation: bool,
    /// Send scale. Off by default: few things rescale, and it is a third of
    /// every update when they do not.
    pub sync_scale: bool,
    /// Who moves it. `#[reflect(default)]` because scenes saved before the
    /// field existed carry a `NetworkTransform` without it.
    #[reflect(default)]
    #[serde(default)]
    pub authority: TransformAuthority,
}

impl Default for NetworkTransform {
    fn default() -> Self {
        Self {
            interpolate: true,
            sync_rotation: true,
            sync_scale: false,
            authority: TransformAuthority::Server,
        }
    }
}

impl NetworkTransform {
    /// The settings a player avatar wants: moved by its owner, smoothed for
    /// everyone else.
    pub fn owner_driven() -> Self {
        Self { authority: TransformAuthority::Owner, ..Self::default() }
    }
}

/// What a networked entity looks like: a scene (`.bsn`, project-relative)
/// that every peer instances under it locally.
///
/// Replicated, so the server sends a path and each client loads the prefab
/// from its own copy of the project. Sending the meshes and materials
/// themselves would cost megabytes per spawn; every peer already has them.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkPrefab(pub String);

/// Spawn a player avatar for every peer that joins.
///
/// Placed in a scene; only the server acts on it. The avatar is an instance of
/// `prefab` (a `.bsn` scene, project-relative), placed around this entity on a
/// ring of `spawn_radius`, owned by the peer it is for, and moved by that peer.
/// On a listen server (`--host`) the host gets one too.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_network::components"]
pub struct NetworkPlayerSpawner {
    /// The avatar scene, relative to the project root, e.g. `prefabs/player.bsn`.
    pub prefab: String,
    /// Players are spread around a ring of this radius so two joining at once
    /// do not spawn inside each other. Zero spawns everyone on the spawner.
    pub spawn_radius: f32,
}

impl Default for NetworkPlayerSpawner {
    fn default() -> Self {
        Self { prefab: String::new(), spawn_radius: 2.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_defaults_to_server() {
        assert_eq!(NetworkOwner::default().0, OwnerKind::Server);
    }

    #[test]
    fn a_network_transform_saved_before_authority_existed_still_loads() {
        let old = r#"{"interpolate":true,"sync_rotation":false,"sync_scale":false}"#;
        let t: NetworkTransform = serde_json::from_str(old).unwrap();
        assert_eq!(t.authority, TransformAuthority::Server);
        assert!(!t.sync_rotation);
    }

    #[test]
    fn type_paths_did_not_move_with_the_types() {
        assert_eq!(<Networked as TypePath>::type_path(), "renzora_network::components::Networked");
        assert_eq!(
            <NetworkTransform as TypePath>::type_path(),
            "renzora_network::components::NetworkTransform"
        );
    }
}
