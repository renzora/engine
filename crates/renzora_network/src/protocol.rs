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
    // Channels. `add_direction` is required, not optional: it installs the
    // observers that attach each channel's sender/receiver to a link's
    // `Transport` when the connection is established. Without it the channel
    // sits in the registry but is wired to no transport, and any send fails
    // with `ChannelNotFound`. Both channels are bidirectional (client and
    // server both send and receive RPCs/state).
    app.add_channel::<ReliableChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    })
    .add_direction(NetworkDirection::Bidirectional);
    app.add_channel::<UnreliableChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedUnreliable,
        ..default()
    })
    .add_direction(NetworkDirection::Bidirectional);

    // Replicated components (server → client)
    app.register_component::<Networked>();
    app.register_component::<NetworkId>();
    app.register_component::<Name>();
    // Player avatars: clients need the marker (to attach a visual) and the
    // owner (to know whose avatar it is / tint it). Without registering these,
    // the server-spawned avatar arrives bare and `Added<NetworkPlayer>` never
    // fires on the client.
    app.register_component::<NetworkPlayer>();
    app.register_component::<NetworkOwner>();

    // Transform is the core replicated state — position/rotation of every
    // networked entity. `add_interpolation_with` enables the interpolation
    // systems so entities tagged `InterpolationTarget` (see server.rs) move
    // smoothly between snapshots instead of snapping at the tick rate.
    // `TransformLinearInterpolation::lerp` does translation lerp + rotation
    // slerp; lightyear ships it for exactly this.
    app.register_component::<Transform>()
        .add_interpolation_with(TransformLinearInterpolation::lerp);

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
