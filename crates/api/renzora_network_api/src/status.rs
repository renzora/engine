//! What the session looks like right now: written by `renzora_network` every
//! frame, read by scripts, the editor's network panels and the HUD.

use bevy::prelude::*;

/// The peer id the server itself uses, as the sender of an RPC it makes and
/// as an owner. Never handed to a client.
pub const SERVER_PEER_ID: u64 = 0;

/// The host's own player on a listen server (`--host`). Fixed, so a host's
/// scripts can name themselves without asking. Never handed to a remote
/// client: the server draws client ids from the rest of the range.
pub const HOST_PEER_ID: u64 = 1;

/// What this process is in the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetRole {
    /// Not in a session. Single-player, or a client that has not connected.
    #[default]
    Offline,
    /// Connected (or connecting) to a server somewhere else.
    Client,
    /// A headless dedicated server (`--server`). Has no player of its own.
    DedicatedServer,
    /// A listen server (`--host`): the server, plus a local player with
    /// [`HOST_PEER_ID`].
    Host,
}

impl NetRole {
    /// Whether this process runs the authoritative simulation.
    pub fn is_server(self) -> bool {
        matches!(self, Self::DedicatedServer | Self::Host)
    }
}

/// Connection state for the local endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

/// One remote peer, as the server sees it.
#[derive(Debug, Clone, Default)]
pub struct ConnectedClient {
    pub client_id: u64,
    pub rtt_ms: f32,
    /// Fraction of packets to this peer that were never acknowledged, over
    /// roughly the last second.
    pub packet_loss: f32,
}

/// The session, as last observed. One resource on every peer.
#[derive(Resource, Debug, Clone, Default)]
pub struct NetworkStatus {
    pub role: NetRole,
    pub state: ConnectionState,
    /// Kept beside `role` for the code written before roles existed.
    pub is_server: bool,
    /// This peer's id: [`HOST_PEER_ID`] on a host, [`SERVER_PEER_ID`] on a
    /// dedicated server, the id the server assigned on a client.
    pub client_id: Option<u64>,
    /// Round trip to the server, smoothed (client only).
    pub rtt_ms: f32,
    /// How much the round trip varies, smoothed (client only).
    pub jitter_ms: f32,
    /// Fraction of sent packets never acknowledged (client only).
    pub packet_loss: f32,
    /// Bytes per second, both directions, over the last second.
    pub bytes_in_per_sec: f32,
    pub bytes_out_per_sec: f32,
    /// The server's simulation tick, as last known.
    pub tick: u32,
    /// Remote peers (server only).
    pub connected_clients: Vec<ConnectedClient>,
    /// Every player in the session, this one included, as the server last
    /// reported it. On the server this is the host (if any) plus
    /// `connected_clients`; on a client it is the roster the server sends.
    pub players: Vec<u64>,
    /// Entities currently replicated.
    pub replicated_entities: usize,
    /// Why the last connection attempt ended, if it did not end by choice.
    pub last_error: Option<String>,
}

impl NetworkStatus {
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    pub fn client_count(&self) -> usize {
        self.connected_clients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_offline_and_disconnected() {
        let s = NetworkStatus::default();
        assert_eq!(s.role, NetRole::Offline);
        assert!(!s.is_connected());
        assert_eq!(s.client_count(), 0);
    }

    #[test]
    fn host_and_dedicated_are_both_servers() {
        assert!(NetRole::Host.is_server());
        assert!(NetRole::DedicatedServer.is_server());
        assert!(!NetRole::Client.is_server());
        assert!(!NetRole::Offline.is_server());
    }
}
