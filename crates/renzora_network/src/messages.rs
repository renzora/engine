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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_request_round_trip() {
        let msg = SpawnRequest {
            name: "enemy".to_string(),
            position: Vec3::new(1.0, 2.0, 3.0),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: SpawnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, msg.name);
        assert_eq!(back.position, msg.position);
    }

    #[test]
    fn despawn_request_round_trip() {
        let msg = DespawnRequest { network_id: 99 };
        let json = serde_json::to_string(&msg).unwrap();
        let back: DespawnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.network_id, 99);
    }

    #[test]
    fn chat_message_round_trip() {
        let msg = ChatMessage {
            sender: "alice".to_string(),
            content: "hello world".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sender, msg.sender);
        assert_eq!(back.content, msg.content);
    }

    #[test]
    fn game_event_round_trip_with_binary_payload() {
        let msg = GameEvent {
            name: "score".to_string(),
            data: vec![0u8, 1, 2, 255, 128],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: GameEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, msg.name);
        assert_eq!(back.data, msg.data);
    }

    #[test]
    fn game_event_empty_payload() {
        let msg = GameEvent {
            name: "ping".to_string(),
            data: Vec::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: GameEvent = serde_json::from_str(&json).unwrap();
        assert!(back.data.is_empty());
        assert_eq!(back.name, "ping");
    }
}
