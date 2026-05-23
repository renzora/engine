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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networked_default_and_eq() {
        assert_eq!(Networked, Networked::default());
    }

    #[test]
    fn network_owner_default_is_server() {
        let owner = NetworkOwner::default();
        assert_eq!(owner.0, OwnerKind::Server);
    }

    #[test]
    fn network_id_round_trip() {
        let id = NetworkId(42);
        let json = serde_json::to_string(&id).unwrap();
        let back: NetworkId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn owner_kind_round_trip_both_variants() {
        for kind in [OwnerKind::Server, OwnerKind::Client(7)] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: OwnerKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn owner_kind_variants_distinct() {
        assert_ne!(OwnerKind::Server, OwnerKind::Client(0));
        assert_ne!(OwnerKind::Client(1), OwnerKind::Client(2));
        assert_eq!(OwnerKind::Client(5), OwnerKind::Client(5));
    }

    #[test]
    fn networked_round_trip() {
        let json = serde_json::to_string(&Networked).unwrap();
        let back: Networked = serde_json::from_str(&json).unwrap();
        assert_eq!(Networked, back);
    }
}
