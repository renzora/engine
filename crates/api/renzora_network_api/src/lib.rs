//! The multiplayer contract: `renzora::network` to a Rust script or plugin.
//!
//! What lives here is everything a game needs to *say* about multiplayer:
//!
//! - [`components`]: what a scene marks. [`Networked`] replicates an entity,
//!   [`NetworkOwner`] says whose it is, [`NetworkTransform`] says who moves it,
//!   [`NetworkPlayerSpawner`] gives every joining peer an avatar.
//! - [`rpc`]: calls between peers, [`Rpc::new("name").arg(..).to(..).send(world)`](Rpc).
//! - [`session`]: questions ([`has_authority`], [`is_mine`], [`is_server`]) and
//!   requests ([`connect`], [`spawn_prefab`]).
//! - [`status`]: the session as last observed, [`NetworkStatus`].
//! - [`registry`]: which components replicate, [`ReplicationRegistry`].
//!
//! What does not: the socket, the protocol, the replication systems. Those are
//! `renzora_network`, an engine feature a single-player export strips. A call
//! made here in such a build queues into a [`NetOutbox`] nothing drains and
//! every [`has_authority`] answers true, so shared code keeps working.

pub mod components;
pub mod registry;
pub mod rpc;
pub mod session;
pub mod status;

pub use components::{
    NetworkId, NetworkOwner, NetworkPlayer, NetworkPlayerSpawner, NetworkPrefab, NetworkTransform,
    Networked, OwnerKind, TransformAuthority,
};
pub use registry::{AppReplicateExt, ReplicationRegistry};
pub use rpc::{Delivery, IntoRpcArg, NetOutbox, NetRequest, OutgoingRpc, Rpc, RpcArgs, RpcTarget};
pub use session::{
    connect, disconnect, entity_by_net_id, has_authority, is_client, is_connected, is_mine,
    is_server, local_peer_id, net_id_of, networked_root, owner_of, players, role, spawn_prefab,
};
pub use status::{
    ConnectedClient, ConnectionState, NetRole, NetworkStatus, HOST_PEER_ID, SERVER_PEER_ID,
};
