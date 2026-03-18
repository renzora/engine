//! Network status resources — observable by editor panels and scripts.

use bevy::prelude::*;

/// Connection state for the local endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

/// Connected client info (server-side).
#[derive(Debug, Clone)]
pub struct ConnectedClient {
    pub client_id: u64,
    pub rtt_ms: f32,
}

/// Observable network status resource — updated each frame by network systems.
#[derive(Resource, Default, Debug)]
pub struct NetworkStatus {
    /// Current connection state.
    pub state: ConnectionState,
    /// Whether this instance is running as a server.
    pub is_server: bool,
    /// Local client ID (only meaningful on client).
    pub client_id: Option<u64>,
    /// Round-trip time in milliseconds (client only).
    pub rtt_ms: f32,
    /// Jitter in milliseconds (client only).
    pub jitter_ms: f32,
    /// Packet loss ratio 0.0..1.0 (client only).
    pub packet_loss: f32,
    /// Connected clients (server only).
    pub connected_clients: Vec<ConnectedClient>,
}

impl NetworkStatus {
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    pub fn client_count(&self) -> usize {
        self.connected_clients.len()
    }
}
