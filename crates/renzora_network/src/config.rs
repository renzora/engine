//! Network configuration types.

use serde::{Deserialize, Serialize};

/// Transport protocol for networking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TransportKind {
    #[default]
    Udp,
    WebTransport,
    WebSocket,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_default_is_udp() {
        assert_eq!(TransportKind::default(), TransportKind::Udp);
    }

    #[test]
    fn transport_from_str_loose_aliases() {
        assert_eq!(
            TransportKind::from_str_loose("webtransport"),
            TransportKind::WebTransport
        );
        assert_eq!(
            TransportKind::from_str_loose("web_transport"),
            TransportKind::WebTransport
        );
        assert_eq!(
            TransportKind::from_str_loose("websocket"),
            TransportKind::WebSocket
        );
        assert_eq!(
            TransportKind::from_str_loose("web_socket"),
            TransportKind::WebSocket
        );
        assert_eq!(TransportKind::from_str_loose("ws"), TransportKind::WebSocket);
        assert_eq!(TransportKind::from_str_loose("udp"), TransportKind::Udp);
    }

    #[test]
    fn transport_from_str_loose_is_case_insensitive() {
        assert_eq!(
            TransportKind::from_str_loose("WebSocket"),
            TransportKind::WebSocket
        );
        assert_eq!(
            TransportKind::from_str_loose("WEBTRANSPORT"),
            TransportKind::WebTransport
        );
    }

    #[test]
    fn transport_from_str_loose_unknown_falls_back_to_udp() {
        assert_eq!(TransportKind::from_str_loose(""), TransportKind::Udp);
        assert_eq!(TransportKind::from_str_loose("garbage"), TransportKind::Udp);
        assert_eq!(TransportKind::from_str_loose("tcp"), TransportKind::Udp);
    }

    #[test]
    fn transport_serializes_lowercase() {
        // #[serde(rename_all = "lowercase")] => variant names lowercased.
        assert_eq!(
            serde_json::to_string(&TransportKind::Udp).unwrap(),
            "\"udp\""
        );
        assert_eq!(
            serde_json::to_string(&TransportKind::WebTransport).unwrap(),
            "\"webtransport\""
        );
        assert_eq!(
            serde_json::to_string(&TransportKind::WebSocket).unwrap(),
            "\"websocket\""
        );
    }

    #[test]
    fn transport_round_trip() {
        for kind in [
            TransportKind::Udp,
            TransportKind::WebTransport,
            TransportKind::WebSocket,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: TransportKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn network_config_defaults() {
        let c = NetworkConfig::default();
        assert_eq!(c.server_addr, "127.0.0.1");
        assert_eq!(c.port, 7636);
        assert_eq!(c.transport, TransportKind::Udp);
        assert_eq!(c.tick_rate, 64);
        assert_eq!(c.max_clients, 32);
    }

    #[test]
    fn network_config_round_trip() {
        let c = NetworkConfig {
            server_addr: "example.com".to_string(),
            port: 9000,
            transport: TransportKind::WebTransport,
            tick_rate: 30,
            max_clients: 8,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: NetworkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.server_addr, c.server_addr);
        assert_eq!(back.port, c.port);
        assert_eq!(back.transport, c.transport);
        assert_eq!(back.tick_rate, c.tick_rate);
        assert_eq!(back.max_clients, c.max_clients);
    }

    #[test]
    fn network_config_default_round_trip() {
        let c = NetworkConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: NetworkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.server_addr, c.server_addr);
        assert_eq!(back.port, c.port);
        assert_eq!(back.transport, c.transport);
        assert_eq!(back.tick_rate, c.tick_rate);
        assert_eq!(back.max_clients, c.max_clients);
    }
}
