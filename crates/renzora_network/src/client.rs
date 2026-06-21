//! Client-side networking over the from-scratch UDP transport.
//!
//! A connection is created on demand from `PendingNetworkConnect` (set by
//! scripts/blueprints). [`NetworkClient`] holds the socket + the server peer;
//! [`client_poll`] pumps it each frame, delivering received RPCs to scripts.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::messages::GameEvent;
use crate::status::{ConnectionState, NetworkStatus};
use crate::transport::{decode, encode, Packet, Peer, MAX_DATAGRAM};

/// How often to re-send a `ConnectRequest` while waiting to be accepted.
const CONNECT_RETRY: Duration = Duration::from_millis(250);

/// Generate a pseudo-random client ID from system time.
pub fn rand_client_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42)
}

/// The active client connection. Inserted by `process_pending_connect`, removed
/// on disconnect. Its presence (and `connected`) drives [`NetworkStatus`].
#[derive(Resource)]
pub struct NetworkClient {
    socket: UdpSocket,
    server: Peer,
    pub client_id: u64,
    pub connected: bool,
    last_request: Instant,
}

impl NetworkClient {
    /// Bind a local UDP socket and begin the handshake with `server_addr`.
    pub fn connect(server_addr: SocketAddr, client_id: u64) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let mut client = Self {
            socket,
            server: Peer::new(server_addr, client_id),
            client_id,
            connected: false,
            last_request: Instant::now(),
        };
        client.send_connect();
        Ok(client)
    }

    fn send_connect(&mut self) {
        let _ = self.socket.send_to(
            &encode(&Packet::ConnectRequest { client_id: self.client_id }),
            self.server.addr,
        );
        self.last_request = Instant::now();
    }

    /// Poll the socket; returns the `GameEvent`s received this frame (already
    /// de-duplicated). Also drives the handshake, acks, and resends.
    pub fn update(&mut self) -> Vec<GameEvent> {
        let mut delivered = Vec::new();
        let mut buf = [0u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, from)) if from == self.server.addr => {
                    self.server.last_recv = Instant::now();
                    let Some(packet) = decode(&buf[..n]) else { continue };
                    match packet {
                        Packet::ConnectAccept { .. } => self.connected = true,
                        Packet::Reliable { seq, event } => {
                            if self.server.on_reliable(&self.socket, seq) {
                                delivered.push(event);
                            }
                        }
                        Packet::Ack { seq } => self.server.on_ack(seq),
                        Packet::Disconnect => self.connected = false,
                        Packet::ConnectRequest { .. } | Packet::KeepAlive => {}
                    }
                }
                // Datagram from someone other than our server — ignore.
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        // Keep retrying the handshake until accepted.
        if !self.connected && self.last_request.elapsed() >= CONNECT_RETRY {
            self.send_connect();
        }
        self.server.tick(&self.socket);
        delivered
    }

    /// Reliably send a `GameEvent` to the server (no-op until connected).
    pub fn send_event(&mut self, event: GameEvent) {
        if self.connected {
            self.server.send_reliable(&self.socket, event);
        }
    }

    /// Best-effort graceful close (a single Disconnect datagram).
    pub fn disconnect(&mut self) {
        let _ = self.socket.send_to(&encode(&Packet::Disconnect), self.server.addr);
    }
}

/// Pump the client connection each frame: deliver received RPCs to scripts.
pub fn client_poll(
    client: Option<ResMut<NetworkClient>>,
    mut inbox: ResMut<renzora::ScriptRpcInbox>,
) {
    let Some(mut client) = client else { return };
    for event in client.update() {
        inbox.pending.push(renzora::IncomingRpc {
            name: event.name,
            args: crate::rpc::args_from_bytes(&event.data),
            from: 0, // 0 = "from server/local" in script space
        });
    }
}

/// Mirror the connection state into the observable [`NetworkStatus`]. On a
/// server (no `NetworkClient`) the status stays `Connected`, so we only flip to
/// `Disconnected` when we're not a server.
pub fn update_network_status(
    client: Option<Res<NetworkClient>>,
    mut status: ResMut<NetworkStatus>,
) {
    match client {
        Some(client) if client.connected => {
            if status.state != ConnectionState::Connected {
                status.state = ConnectionState::Connected;
                status.client_id = Some(client.client_id);
                info!("[network] Connected to server");
            }
        }
        Some(_) => {
            if status.state != ConnectionState::Connecting {
                status.state = ConnectionState::Connecting;
            }
        }
        None => {
            if !status.is_server && status.state != ConnectionState::Disconnected {
                status.state = ConnectionState::Disconnected;
                status.client_id = None;
            }
        }
    }
}
