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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum OwnerKind {
    /// Server-authoritative (NPCs, world objects).
    Server,
    /// Owned by a specific client (player characters).
    Client(u64),
}

/// Identifies the owner of a networked entity.
///
/// Usually assigned at runtime by the server (e.g. a player avatar gets
/// `Client(id)` on spawn). World objects placed in the editor default to
/// `Server`. Reflected so it serializes into scenes and shows in the inspector.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NetworkOwner(pub OwnerKind);

impl Default for NetworkOwner {
    fn default() -> Self {
        Self(OwnerKind::Server)
    }
}

/// Marker for a server-spawned player avatar — one per connected client.
///
/// The server spawns these on join (with `Networked` + `NetworkOwner`) and
/// despawns them on leave; replication carries the marker to every client, so
/// a client-side system can give each one a visual. Game code can match on it
/// to attach the real avatar model.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NetworkPlayer;

/// Tunes how a [`Networked`] entity's `Transform` is replicated — the
/// "synchronizer" knobs. Optional: a `Networked` entity with no
/// `NetworkTransform` syncs its transform with these defaults (position +
/// rotation, interpolated on remote peers). Attach this to override.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NetworkTransform {
    /// Smoothly interpolate this entity between replicated snapshots on peers
    /// that don't own it. Off = snap to each received position (jittery, but
    /// useful for debugging or teleport-only entities).
    pub interpolate: bool,
    /// Replicate rotation (translation always replicates).
    pub sync_rotation: bool,
    /// Replicate scale. Off by default — most networked entities don't
    /// rescale, and skipping it saves bandwidth.
    pub sync_scale: bool,
}

impl Default for NetworkTransform {
    fn default() -> Self {
        Self {
            interpolate: true,
            sync_rotation: true,
            sync_scale: false,
        }
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
