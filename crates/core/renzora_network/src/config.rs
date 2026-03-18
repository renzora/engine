//! Network configuration types.

use serde::{Deserialize, Serialize};

/// Transport protocol for networking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    Udp,
    WebTransport,
    WebSocket,
}

impl Default for TransportKind {
    fn default() -> Self {
        Self::Udp
    }
}

impl TransportKind {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "webtransport" | "web_transport" => Self::WebTransport,
            "websocket" | "web_socket" | "ws" => Self::WebSocket,
            _ => Self::Udp,
        }
    }
}

/// Network configuration — read from project config or set at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Server address (IP or hostname).
    pub server_addr: String,
    /// Port for the server to listen on / client to connect to.
    pub port: u16,
    /// Transport protocol.
    pub transport: TransportKind,
    /// Server tick rate in Hz.
    pub tick_rate: u16,
    /// Maximum number of connected clients.
    pub max_clients: u16,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1".to_string(),
            port: 7636,
            transport: TransportKind::Udp,
            tick_rate: 64,
            max_clients: 32,
        }
    }
}
