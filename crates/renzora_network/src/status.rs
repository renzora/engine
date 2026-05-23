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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_state_default_disconnected() {
        assert_eq!(ConnectionState::default(), ConnectionState::Disconnected);
    }

    #[test]
    fn status_default_is_disconnected_no_clients() {
        let s = NetworkStatus::default();
        assert!(!s.is_connected());
        assert!(!s.is_server);
        assert_eq!(s.client_id, None);
        assert_eq!(s.client_count(), 0);
    }

    #[test]
    fn is_connected_tracks_state() {
        let mut s = NetworkStatus::default();
        assert!(!s.is_connected());
        s.state = ConnectionState::Connecting;
        assert!(!s.is_connected());
        s.state = ConnectionState::Connected;
        assert!(s.is_connected());
    }

    #[test]
    fn client_count_matches_vec_len() {
        let mut s = NetworkStatus::default();
        assert_eq!(s.client_count(), 0);
        s.connected_clients.push(ConnectedClient {
            client_id: 1,
            rtt_ms: 12.0,
        });
        s.connected_clients.push(ConnectedClient {
            client_id: 2,
            rtt_ms: 30.0,
        });
        assert_eq!(s.client_count(), 2);
    }
}
