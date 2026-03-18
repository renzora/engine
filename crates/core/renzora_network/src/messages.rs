//! Network message types for the protocol.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Client requests the server to spawn an entity.
#[derive(Debug, Clone, Serialize, Deserialize, Event)]
pub struct SpawnRequest {
    /// Name/tag for the entity to spawn.
    pub name: String,
    /// World position to spawn at.
    pub position: Vec3,
}

/// Client requests the server to despawn an entity.
#[derive(Debug, Clone, Serialize, Deserialize, Event)]
pub struct DespawnRequest {
    /// Network ID of the entity to despawn.
    pub network_id: u64,
}

/// Chat message (bidirectional).
#[derive(Debug, Clone, Serialize, Deserialize, Event)]
pub struct ChatMessage {
    /// Sender name (set by server for client messages).
    pub sender: String,
    /// Message content.
    pub content: String,
}

/// Generic game event (bidirectional, extensible).
///
/// Scripts and blueprints can send/receive arbitrary events
/// using a name + serialized payload.
#[derive(Debug, Clone, Serialize, Deserialize, Event)]
pub struct GameEvent {
    /// Event name for routing.
    pub name: String,
    /// Serialized payload (MessagePack, JSON, or raw bytes).
    pub data: Vec<u8>,
}
