//! The server: accepts players, routes RPCs, and replicates every
//! [`Networked`] entity to every client.
//!
//! # Replication, per send tick
//!
//! 1. Every `Networked` entity is visited. One not seen before gets a
//!    [`NetworkId`] and a `Spawn` to every client. One seen before has its
//!    registered components serialized and compared with what was last sent;
//!    the differences go out as an `Update`. One that disappeared goes out as a
//!    `Despawn`. All three are reliable and ordered.
//! 2. Transforms go in `Snapshot`s, unreliably: only those that moved, plus
//!    each one at least once a second so a lost final update is repaired. An
//!    entity its owner moves is left out of that owner's snapshots: the owner
//!    already knows where it is, and an echo arriving 50 ms late would drag it
//!    backwards.
//! 3. A client that joined since the last tick gets the whole world as spawns,
//!    after the diff, so what it receives is current.
//!
//! The host's own player is not a connection. On a listen server the host is
//! [`HOST_PEER_ID`] and everything addressed to it is delivered in-process.

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Instant;

use bevy::prelude::*;
use renzora_network_api::{
    NetOutbox, NetRequest, NetRole, NetworkId, NetworkOwner, NetworkStatus, NetworkTransform, Networked,
    OwnerKind, ReplicationRegistry, TransformAuthority, ConnectedClient, ConnectionState, HOST_PEER_ID,
    SERVER_PEER_ID,
};
use renzora_scripting_api::{IncomingRpc, NetPlayerEvent, ScriptNetLifecycleInbox, ScriptRpcInbox};

use crate::config::NetworkConfig;
use crate::interpolation::{NetClock, NetInterpolation};
use crate::protocol::{self, NetMessage, SpawnData, WireComponent, WireTarget, WireTransform, TRANSFORMS_PER_MESSAGE};
use crate::reflect_io;
use crate::transport::udp::Socket;
use crate::transport::{Channel, ServerEndpoint, ServerEvent};

/// Snapshots per second, whatever the simulation tick rate. Above this the
/// bandwidth grows and the smoothness does not: interpolation already hides
/// the gaps.
const MAX_SEND_RATE: f64 = 30.0;
/// Resend an unmoving entity's transform this often, so one lost update does
/// not leave a client showing a stale position forever.
const TRANSFORM_REFRESH_SECS: f64 = 1.0;

/// Server half of the network: binds the socket at startup. Added only for
/// `--server` and `--host`, beside [`crate::NetworkPlugin`].
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
        app.insert_resource(ServerNetworkConfig(self.config.clone()));
        app.add_systems(Startup, start_server);
    }
}

#[derive(Resource)]
struct ServerNetworkConfig(NetworkConfig);

struct Record {
    net_id: u64,
    scene_id: Option<u64>,
    owner: OwnerKind,
    owner_moves: bool,
    components: HashMap<String, Vec<u8>>,
    transform: WireTransform,
    last_sent: Option<WireTransform>,
    last_sent_at: f64,
}

/// The running server. Present from startup on `--server` / `--host`.
#[derive(Resource)]
pub struct NetServer {
    endpoint: ServerEndpoint,
    socket: Socket,
    started: Instant,
    last_frame: Instant,
    send_interval: f64,
    next_send: f64,
    /// [`HOST_PEER_ID`] on a listen server, [`SERVER_PEER_ID`] on a dedicated one.
    local_peer: u64,
    /// Joined since the last send tick, owed the whole world.
    awaiting_world: Vec<u64>,
    records: HashMap<Entity, Record>,
    by_net_id: HashMap<u64, Entity>,
    next_net_id: u64,
    /// The newest transform each owner reported for its entity, passed on to
    /// everyone else as is. The server's own copy is interpolated for drawing
    /// on a host; relaying that copy would add its delay a second time.
    owner_reports: HashMap<u64, WireTransform>,
}

impl NetServer {
    /// Where the server is listening. With port 0 in the config the OS picks
    /// one, and this is how to find out which.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.local_addr()
    }

    fn is_host(&self) -> bool {
        self.local_peer == HOST_PEER_ID
    }

    /// Everyone in the session.
    fn roster(&self) -> Vec<u64> {
        let mut players: Vec<u64> = Vec::new();
        if self.is_host() {
            players.push(HOST_PEER_ID);
        }
        let mut clients: Vec<u64> = self.endpoint.clients().collect();
        clients.sort_unstable();
        players.extend(clients);
        players
    }

    fn send_to(&mut self, client: u64, channel: Channel, msg: &NetMessage) {
        self.endpoint.send(client, channel, protocol::encode(msg));
    }

    fn broadcast(&mut self, channel: Channel, msg: &NetMessage, except: &[u64]) {
        let bytes = protocol::encode(msg);
        let clients: Vec<u64> = self.endpoint.clients().filter(|c| !except.contains(c)).collect();
        for c in clients {
            self.endpoint.send(c, channel, bytes.clone());
        }
    }
}

/// The game's protocol id: the project's name mixed with the wire version, so
/// a client of another game, or of an incompatible build of this one, is
/// turned away at the handshake instead of misreading the stream.
pub fn protocol_id(world: &World) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let name = world
        .get_resource::<renzora_project_api::CurrentProject>()
        .map(|p| p.config.name.clone())
        .unwrap_or_default();
    name.hash(&mut h);
    crate::transport::packet::MAGIC.hash(&mut h);
    h.finish()
}

fn start_server(world: &mut World) {
    let Some(config) = world.get_resource::<ServerNetworkConfig>().map(|c| c.0.clone()) else { return };
    let host = world.contains_resource::<renzora_project_api::HostServer>();
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let socket = match Socket::bind(addr) {
        Ok(s) => s,
        Err(e) => {
            error!("[network] Could not listen on UDP port {}: {e}", config.port);
            let mut status = world.resource_mut::<NetworkStatus>();
            status.last_error = Some(format!("could not listen on port {}: {e}", config.port));
            return;
        }
    };
    info!(
        "[network] {} listening on UDP {} (max {} players, {} Hz)",
        if host { "Host" } else { "Dedicated server" },
        addr,
        config.max_clients,
        config.tick_rate
    );
    let now = Instant::now();
    let protocol = protocol_id(world);
    let local_peer = if host { HOST_PEER_ID } else { SERVER_PEER_ID };
    world.insert_resource(NetServer {
        endpoint: ServerEndpoint::new(protocol, config.max_clients as usize, config.tick_rate),
        socket,
        started: now,
        last_frame: now,
        send_interval: 1.0 / (config.tick_rate.max(1) as f64).min(MAX_SEND_RATE),
        next_send: 0.0,
        local_peer,
        awaiting_world: Vec::new(),
        records: HashMap::new(),
        by_net_id: HashMap::new(),
        next_net_id: 1,
        owner_reports: HashMap::new(),
    });
    world.insert_resource(NetClock { now: 0.0, synced: true });
    let mut status = world.resource_mut::<NetworkStatus>();
    status.role = if host { NetRole::Host } else { NetRole::DedicatedServer };
    status.is_server = true;
    status.state = ConnectionState::Connected;
    status.client_id = Some(local_peer);
    status.players = if host { vec![HOST_PEER_ID] } else { Vec::new() };
    if host {
        world.resource_mut::<crate::spawner::PendingAvatars>().0.push(HOST_PEER_ID);
    }
}

/// Deliver an RPC to this process's own scripts.
fn deliver_locally(world: &mut World, from: u64, name: &str, args: &[u8]) {
    world.resource_mut::<ScriptRpcInbox>().pending.push(IncomingRpc {
        name: name.to_string(),
        args: protocol::args_from_bytes(args),
        from,
    });
}

/// Send an RPC where its target says, from `from` (already verified).
fn route_rpc(world: &mut World, server: &mut NetServer, from: u64, target: WireTarget, channel: Channel, name: String, args: Vec<u8>) {
    let local = server.local_peer;
    let msg = NetMessage::Rpc { from, target, name: name.clone(), args: args.clone() };
    match target {
        WireTarget::Server => deliver_locally(world, from, &name, &args),
        WireTarget::All => {
            deliver_locally(world, from, &name, &args);
            server.broadcast(channel, &msg, &[]);
        }
        WireTarget::Others => {
            if from != local {
                deliver_locally(world, from, &name, &args);
            }
            server.broadcast(channel, &msg, &[from]);
        }
        WireTarget::Peer(p) if p == local => deliver_locally(world, from, &name, &args),
        WireTarget::Peer(p) => server.send_to(p, channel, &msg),
    }
}

/// Read the socket and act on what arrived. First thing in the frame, so
/// scripts see this frame's RPCs and joins.
pub fn server_receive(world: &mut World) {
    let Some(mut server) = world.remove_resource::<NetServer>() else { return };
    let now = Instant::now();
    {
        let NetServer { socket, endpoint, .. } = &mut server;
        socket.drain(|from, bytes| endpoint.receive(from, bytes, now));
    }
    world.resource_mut::<NetClock>().now = now.duration_since(server.started).as_secs_f64();
    server.last_frame = now;

    let mut roster_changed = false;
    for event in server.endpoint.take_events() {
        match event {
            ServerEvent::Connected(id) => {
                info!("[network] Player {id} joined");
                server.awaiting_world.push(id);
                world.resource_mut::<ScriptNetLifecycleInbox>().pending.push(NetPlayerEvent { id, joined: true });
                world.resource_mut::<crate::spawner::PendingAvatars>().0.push(id);
                roster_changed = true;
            }
            ServerEvent::Disconnected(id) => {
                info!("[network] Player {id} left");
                server.awaiting_world.retain(|c| *c != id);
                world.resource_mut::<ScriptNetLifecycleInbox>().pending.push(NetPlayerEvent { id, joined: false });
                world.resource_mut::<crate::spawner::PendingAvatars>().0.retain(|c| *c != id);
                despawn_owned_by(world, id);
                roster_changed = true;
            }
            ServerEvent::Message(from, channel, bytes) => {
                let Some(msg) = protocol::decode(&bytes) else { continue };
                match msg {
                    // `from` is the connection's id, whatever the message
                    // claims: one player must not be able to speak as another.
                    NetMessage::Rpc { target, name, args, .. } => {
                        route_rpc(world, &mut server, from, target, channel, name, args)
                    }
                    NetMessage::Owned { time, transforms } => {
                        apply_owner_reports(world, &mut server, from, time, transforms)
                    }
                    // Only the server sends the rest.
                    _ => {}
                }
            }
        }
    }
    if roster_changed {
        let players = server.roster();
        server.broadcast(Channel::Reliable, &NetMessage::Roster { players: players.clone() }, &[]);
        world.resource_mut::<NetworkStatus>().players = players;
    }
    world.insert_resource(server);
}

/// Take a client's word for where its own entities are, for exactly the
/// entities it owns and is allowed to move.
fn apply_owner_reports(world: &mut World, server: &mut NetServer, from: u64, time: f64, transforms: Vec<(u64, WireTransform)>) {
    for (net_id, t) in transforms {
        let Some(&entity) = server.by_net_id.get(&net_id) else { continue };
        let Some(record) = server.records.get(&entity) else { continue };
        if !(record.owner_moves && record.owner == OwnerKind::Client(from)) {
            continue;
        }
        server.owner_reports.insert(net_id, t);
        if let Ok(mut em) = world.get_entity_mut(entity) {
            match em.get_mut::<NetInterpolation>() {
                Some(mut interp) => interp.push(time, t),
                None => {
                    let mut interp = NetInterpolation::new(true);
                    interp.push(time, t);
                    em.insert(interp);
                }
            }
        }
    }
}

fn despawn_owned_by(world: &mut World, client: u64) {
    let mut q = world.query::<(Entity, &NetworkOwner)>();
    let owned: Vec<Entity> = q
        .iter(world)
        .filter(|(_, o)| o.0 == OwnerKind::Client(client))
        .map(|(e, _)| e)
        .collect();
    for e in owned {
        if let Ok(em) = world.get_entity_mut(e) {
            em.despawn();
        }
    }
}

/// Send RPCs, replicate, and flush the socket. Last thing in the frame, so
/// what scripts did this frame goes out this frame.
pub fn server_send(world: &mut World) {
    let Some(mut server) = world.remove_resource::<NetServer>() else { return };

    let outbox = std::mem::take(&mut *world.resource_mut::<NetOutbox>());
    let local = server.local_peer;
    for rpc in outbox.rpcs {
        let channel = match rpc.delivery {
            renzora_network_api::Delivery::Reliable => Channel::Reliable,
            renzora_network_api::Delivery::Unreliable => Channel::Unreliable,
        };
        let args = protocol::args_to_bytes(&rpc.args);
        route_rpc(world, &mut server, local, rpc.target.into(), channel, rpc.name, args);
    }
    for request in outbox.requests {
        if let NetRequest::Connect { .. } = request {
            warn!("[network] connect() ignored: this process is the server");
        }
    }

    let clock = world.resource::<NetClock>().now;
    if clock >= server.next_send {
        // Fall behind by more than a tick (a long frame) and the schedule
        // restarts from now rather than bursting to catch up.
        server.next_send = if clock - server.next_send > server.send_interval {
            clock + server.send_interval
        } else {
            server.next_send + server.send_interval
        };
        replicate(world, &mut server, clock);
        let joiners = std::mem::take(&mut server.awaiting_world);
        for client in joiners {
            send_world(&mut server, client);
        }
    }

    let now = Instant::now();
    server.endpoint.update(now);
    for (to, bytes) in server.endpoint.take_outgoing() {
        server.socket.send(to, &bytes);
    }

    let clients: Vec<ConnectedClient> = server
        .endpoint
        .clients()
        .map(|id| {
            let s = server.endpoint.stats(id).unwrap_or_default();
            ConnectedClient { client_id: id, rtt_ms: s.rtt_ms, packet_loss: s.packet_loss }
        })
        .collect();
    let (bytes_in, bytes_out) = server
        .endpoint
        .clients()
        .filter_map(|id| server.endpoint.stats(id))
        .fold((0.0, 0.0), |(i, o), s| (i + s.bytes_in_per_sec, o + s.bytes_out_per_sec));
    {
        let mut status = world.resource_mut::<NetworkStatus>();
        status.connected_clients = clients;
        status.bytes_in_per_sec = bytes_in;
        status.bytes_out_per_sec = bytes_out;
        status.replicated_entities = server.records.len();
        status.tick = (clock / server.send_interval) as u32;
    }
    world.insert_resource(server);
}

/// A networked entity as one replication pass reads it: the entity, its id if
/// it has one, its scene id, and how it moves and whose it is.
type LiveEntity = (Entity, Option<u64>, Option<u64>, Transform, NetworkTransform, OwnerKind);

/// One replication pass. See the module doc.
fn replicate(world: &mut World, server: &mut NetServer, clock: f64) {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let paths: Vec<String> = world
        .get_resource::<ReplicationRegistry>()
        .map(|r| r.paths().to_vec())
        .unwrap_or_else(|| ReplicationRegistry::default().paths().to_vec());

    let mut q = world.query_filtered::<(
        Entity,
        Option<&NetworkId>,
        Option<&renzora_scene_api::SceneEntityId>,
        Option<&Transform>,
        Option<&NetworkTransform>,
        Option<&NetworkOwner>,
    ), With<Networked>>();
    let live: Vec<LiveEntity> = q
        .iter(world)
        .map(|(e, id, scene, t, nt, owner)| {
            (
                e,
                id.map(|i| i.0),
                scene.map(|s| s.0).filter(|s| *s != 0),
                t.copied().unwrap_or_default(),
                nt.cloned().unwrap_or_default(),
                owner.map(|o| o.0).unwrap_or_default(),
            )
        })
        .collect();

    let awaiting: Vec<u64> = server.awaiting_world.clone();
    let mut spawns = Vec::new();
    let mut updates = Vec::new();
    let mut snapshot: Vec<(u64, WireTransform, Option<u64>)> = Vec::new();
    let mut seen = HashSet::new();

    for (entity, existing_id, scene_id, transform, settings, owner) in live {
        seen.insert(entity);
        let owner_moves = settings.authority == TransformAuthority::Owner && matches!(owner, OwnerKind::Client(_));
        let components = reflect_io::read_components(world, &registry, entity, &paths);
        let mut wire = WireTransform::from_transform(&transform, settings.sync_rotation, settings.sync_scale);

        let record = match server.records.get_mut(&entity) {
            Some(record) => record,
            None => {
                let net_id = existing_id.unwrap_or_else(|| {
                    let id = server.next_net_id;
                    server.next_net_id += 1;
                    id
                });
                if existing_id.is_none() {
                    world.entity_mut(entity).insert(NetworkId(net_id));
                }
                spawns.push(SpawnData { net_id, scene_id, transform: wire, components: components.clone() });
                server.by_net_id.insert(net_id, entity);
                server.records.insert(
                    entity,
                    Record {
                        net_id,
                        scene_id,
                        owner,
                        owner_moves,
                        components: components.into_iter().collect(),
                        transform: wire,
                        last_sent: Some(wire),
                        last_sent_at: clock,
                    },
                );
                continue;
            }
        };

        // Components: what changed and what went away since the last pass.
        let mut changed: Vec<WireComponent> = Vec::new();
        let mut now_present = HashSet::new();
        for (path, bytes) in components {
            now_present.insert(path.clone());
            if record.components.get(&path) != Some(&bytes) {
                record.components.insert(path.clone(), bytes.clone());
                changed.push((path, bytes));
            }
        }
        let removed: Vec<String> = record.components.keys().filter(|p| !now_present.contains(*p)).cloned().collect();
        for p in &removed {
            record.components.remove(p);
        }
        if !changed.is_empty() || !removed.is_empty() {
            updates.push(NetMessage::Update { net_id: record.net_id, changed, removed });
        }
        record.owner = owner;
        record.owner_moves = owner_moves;

        // Transform: the owner's own report for an owner-moved entity.
        if owner_moves {
            if let Some(reported) = server.owner_reports.get(&record.net_id) {
                wire = *reported;
            }
        }
        record.transform = wire;
        let moved = record.last_sent.is_none_or(|last| last.differs(&wire));
        if moved || clock - record.last_sent_at >= TRANSFORM_REFRESH_SECS {
            record.last_sent = Some(wire);
            record.last_sent_at = clock;
            let skip = match owner {
                OwnerKind::Client(c) if owner_moves => Some(c),
                _ => None,
            };
            snapshot.push((record.net_id, wire, skip));
        }
    }

    // Whatever was replicated and is no longer here (despawned, or no longer
    // `Networked`) is despawned everywhere.
    let gone: Vec<Entity> = server.records.keys().filter(|e| !seen.contains(*e)).copied().collect();
    let mut despawns = Vec::new();
    for e in gone {
        if let Some(record) = server.records.remove(&e) {
            server.by_net_id.remove(&record.net_id);
            server.owner_reports.remove(&record.net_id);
            despawns.push(NetMessage::Despawn { net_id: record.net_id });
        }
    }

    for msg in spawns.into_iter().map(NetMessage::Spawn).chain(updates).chain(despawns) {
        server.broadcast(Channel::Reliable, &msg, &awaiting);
    }

    if !snapshot.is_empty() {
        let clients: Vec<u64> = server.endpoint.clients().filter(|c| !awaiting.contains(c)).collect();
        for client in clients {
            let for_client: Vec<(u64, WireTransform)> = snapshot
                .iter()
                .filter(|(_, _, skip)| *skip != Some(client))
                .map(|(id, t, _)| (*id, *t))
                .collect();
            for chunk in for_client.chunks(TRANSFORMS_PER_MESSAGE) {
                server.send_to(client, Channel::Unreliable, &NetMessage::Snapshot { time: clock, transforms: chunk.to_vec() });
            }
        }
    }
}

/// Everything, to a client that just joined.
fn send_world(server: &mut NetServer, client: u64) {
    let spawns: Vec<NetMessage> = server
        .records
        .values()
        .map(|r| {
            NetMessage::Spawn(SpawnData {
                net_id: r.net_id,
                scene_id: r.scene_id,
                transform: r.transform,
                components: r.components.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            })
        })
        .collect();
    let roster = NetMessage::Roster { players: server.roster() };
    server.send_to(client, Channel::Reliable, &roster);
    for msg in spawns {
        server.send_to(client, Channel::Reliable, &msg);
    }
}

/// Close every connection cleanly when the app exits, so clients see "the
/// server closed" at once instead of a timeout eight seconds later.
pub fn server_shutdown(mut exit: MessageReader<AppExit>, server: Option<ResMut<NetServer>>) {
    if exit.read().next().is_none() {
        return;
    }
    let Some(mut server) = server else { return };
    server.endpoint.shutdown();
    for (to, bytes) in server.endpoint.take_outgoing() {
        server.socket.send(to, &bytes);
    }
}
