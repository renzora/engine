//! Networked entity components.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker component for entities that should be replicated over the network.
///
/// Add this to any entity that needs to be synchronized between server and clients.
/// The server auto-inserts Lightyear `Replicate` when this is detected.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Networked;

/// Unique network-wide identifier for a replicated entity.
///
/// Assigned by the server on spawn. Clients use this to correlate
/// local entities with server-side entities.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NetworkId(pub u64);

/// Who owns this networked entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnerKind {
    /// Server-authoritative (NPCs, world objects).
    Server,
    /// Owned by a specific client (player characters).
    Client(u64),
}

/// Identifies the owner of a networked entity.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct NetworkOwner(pub OwnerKind);

impl Default for NetworkOwner {
    fn default() -> Self {
        Self(OwnerKind::Server)
    }
}
