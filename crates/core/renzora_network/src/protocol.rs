//! Lightyear protocol definition — channels, components, messages.
//!
//! This module defines what data flows over the network and how.
//! Must be added AFTER Client/Server Plugins but BEFORE spawning entities.

use crate::components::*;
use crate::messages::*;

use bevy::prelude::*;
use lightyear::prelude::*;

// ── Channels ──────────────────────────────────────────────────────────────

/// Reliable ordered channel for spawns, despawns, ownership, config.
pub struct ReliableChannel;

/// Unreliable unordered channel for high-frequency transform updates.
pub struct UnreliableChannel;

// ── Protocol registration ─────────────────────────────────────────────────

/// Register all protocol types (channels, components, messages)
/// with the Lightyear app.
///
/// Must be called after `ServerPlugins`/`ClientPlugins` are added.
pub fn register_protocol(app: &mut App) {
    // Channels
    app.add_channel::<ReliableChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    });
    app.add_channel::<UnreliableChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedUnreliable,
        ..default()
    });

    // Replicated components (server → client)
    app.register_component::<Networked>();
    app.register_component::<NetworkId>();
    app.register_component::<Name>();

    // Messages (as events/triggers)
    app.register_event::<SpawnRequest>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_event::<DespawnRequest>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_event::<ChatMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_event::<GameEvent>()
        .add_direction(NetworkDirection::Bidirectional);
}
