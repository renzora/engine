//! Server-side networking over the from-scratch UDP transport.
//!
//! [`NetworkServer`] owns the listening socket + one [`Peer`] per connected
//! client. [`server_poll`] pumps it each frame: delivering received RPCs to the
//! server's own scripts, fanning them out to the other clients, and reporting
//! join/leave so scripts get `on_player_joined` / `on_player_left`.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;

use bevy::prelude::*;

use crate::components::*;
use crate::config::NetworkConfig;
use crate::messages::GameEvent;
use crate::status::{ConnectedClient, ConnectionState, NetworkStatus};
use crate::transport::{decode, encode, Packet, Peer, MAX_DATAGRAM};

/// Server-side networking plugin. Binds the UDP socket on startup and runs the
/// receive/relay loop. A dedicated server adds only this; a host/listen server
/// adds it alongside `NetworkPlugin` (the host *is* the server — its scripts
/// run server-side, so its RPCs broadcast to clients with no extra local link).
pub struct NetworkServerPlugin {
    pub config: NetworkConfig,
}

impl NetworkServerPlugin {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }
}

impl Plugin for NetworkServerPlugin {
    fn build(&self, app: &mut App) {
        info!("[network] NetworkServerPlugin (port {})", self.config.port);

        app.insert_resource(NetworkStatus {
            is_server: true,
            state: ConnectionState::Connected,
            ..Default::default()
        });
        app.insert_resource(ServerNetworkConfig(self.config.clone()));

        // Server-side scripts can send RPCs (broadcast to all clients). The
        // outbound queue + inboxes are init'd by `NetworkPlugin`, which builds
        // first in every path; it skips its own observer in server mode, so the
        // script-action observer is added here.
        app.add_observer(crate::script_extension::handle_network_script_actions);

        app.add_systems(Startup, start_server);
        app.add_systems(
            Update,
            (server_poll, assign_network_ids, crate::rpc::send_outgoing_rpcs),
        );
    }
}

/// Holds the server config for the startup system.
#[derive(Resource)]
struct ServerNetworkConfig(NetworkConfig);

/// The active server transport. Present once `start_server` has bound the
/// socket; its absence (bind failure) leaves the server inert but compiling.
#[derive(Resource)]
pub struct NetworkServer {
    socket: UdpSocket,
    peers: HashMap<SocketAddr, Peer>,
}

/// What [`NetworkServer::update`] observed this frame.
#[derive(Default)]
pub struct ServerUpdate {
    /// `(sender client_id, event)` for every reliable event received.
    pub events: Vec<(u64, GameEvent)>,
    /// client_ids that connected this frame.
    pub joined: Vec<u64>,
    /// client_ids that disconnected (graceful or timed out) this frame.
    pub left: Vec<u64>,
}

impl NetworkServer {
    /// Bind a non-blocking UDP socket on `0.0.0.0:port`.
    pub fn bind(port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peers: HashMap::new(),
        })
    }

    /// Poll the socket: accept new clients, collect received events, and detect
    /// disconnects/timeouts. Also acks, resends, and keep-alives.
    pub fn update(&mut self) -> ServerUpdate {
        let mut out = ServerUpdate::default();
        let mut buf = [0u8; MAX_DATAGRAM];
        loop {
            let (n, addr) = match self.socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            };
            let Some(packet) = decode(&buf[..n]) else { continue };
            match packet {
                Packet::ConnectRequest { client_id } => {
                    if let std::collections::hash_map::Entry::Vacant(e) = self.peers.entry(addr) {
                        e.insert(Peer::new(addr, client_id));
                        out.joined.push(client_id);
                        info!("[network] Client connected: id={} addr={}", client_id, addr);
                    }
                    // Always (re-)accept — the accept datagram may have been lost.
                    if let Some(peer) = self.peers.get_mut(&addr) {
                        peer.last_recv = Instant::now();
                        let _ = self.socket.send_to(
                            &encode(&Packet::ConnectAccept { client_id }),
                            addr,
                        );
                    }
                }
                Packet::Reliable { seq, event } => {
                    if let Some(peer) = self.peers.get_mut(&addr) {
                        peer.last_recv = Instant::now();
                        if peer.on_reliable(&self.socket, seq) {
                            out.events.push((peer.client_id, event));
                        }
                    }
                }
                Packet::Ack { seq } => {
                    if let Some(peer) = self.peers.get_mut(&addr) {
                        peer.last_recv = Instant::now();
                        peer.on_ack(seq);
                    }
                }
                Packet::KeepAlive => {
                    if let Some(peer) = self.peers.get_mut(&addr) {
                        peer.last_recv = Instant::now();
                    }
                }
                Packet::Disconnect => {
                    if let Some(peer) = self.peers.remove(&addr) {
                        info!("[network] Client disconnected: id={}", peer.client_id);
                        out.left.push(peer.client_id);
                    }
                }
                Packet::ConnectAccept { .. } => {}
            }
        }

        // Time out silent peers.
        let timed_out: Vec<SocketAddr> = self
            .peers
            .iter()
            .filter(|(_, p)| p.timed_out())
            .map(|(a, _)| *a)
            .collect();
        for addr in timed_out {
            if let Some(peer) = self.peers.remove(&addr) {
                info!("[network] Client timed out: id={}", peer.client_id);
                out.left.push(peer.client_id);
            }
        }

        for peer in self.peers.values_mut() {
            peer.tick(&self.socket);
        }
        out
    }

    /// Reliably send an event to every connected client, optionally skipping
    /// one (used to avoid echoing a relayed RPC back to its sender).
    pub fn broadcast(&mut self, event: GameEvent, except: Option<u64>) {
        for peer in self.peers.values_mut() {
            if Some(peer.client_id) != except {
                peer.send_reliable(&self.socket, event.clone());
            }
        }
    }
}

/// Bind the server socket at startup.
fn start_server(mut commands: Commands, config: Res<ServerNetworkConfig>) {
    match NetworkServer::bind(config.0.port) {
        Ok(server) => {
            info!("[network] Server listening on 0.0.0.0:{}", config.0.port);
            commands.insert_resource(server);
        }
        Err(e) => error!("[network] Failed to bind server socket: {e}"),
    }
}

/// Per-frame: pump the server, deliver RPCs to local scripts + relay to other
/// clients, and report join/leave to scripts and status.
fn server_poll(
    server: Option<ResMut<NetworkServer>>,
    mut inbox: ResMut<renzora::ScriptRpcInbox>,
    mut lifecycle: ResMut<renzora::ScriptNetLifecycleInbox>,
    mut status: ResMut<NetworkStatus>,
) {
    let Some(mut server) = server else { return };
    let update = server.update();

    for (from, event) in update.events {
        inbox.pending.push(renzora::IncomingRpc {
            name: event.name.clone(),
            args: crate::rpc::args_from_bytes(&event.data),
            from,
        });
        // Fan out to the other clients (client→server→clients).
        server.broadcast(event, Some(from));
    }

    for id in update.joined {
        status.connected_clients.push(ConnectedClient {
            client_id: id,
            rtt_ms: 0.0,
        });
        lifecycle.pending.push(renzora::NetPlayerEvent { id, joined: true });
    }
    for id in update.left {
        status.connected_clients.retain(|c| c.client_id != id);
        lifecycle.pending.push(renzora::NetPlayerEvent { id, joined: false });
    }
}

/// Assign a `NetworkId` to newly networked entities that lack one. (Full
/// transform/state replication remains TODO — this just stamps the id.)
fn assign_network_ids(
    mut commands: Commands,
    query: Query<Entity, (With<Networked>, Without<NetworkId>)>,
    mut next_id: Local<u64>,
) {
    for entity in &query {
        *next_id += 1;
        commands.entity(entity).insert(NetworkId(*next_id));
    }
}
