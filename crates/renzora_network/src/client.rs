//! The client: joins a server, mirrors what it replicates, and reports where
//! its own avatar is.
//!
//! # Adopting scene entities
//!
//! Client and server load the same scene, so a `Networked` crate placed in the
//! editor already exists on both. A spawn carrying a `scene_id` is matched to
//! the local entity with that `SceneEntityId` and adopted, instead of a second
//! crate appearing beside the first. A spawn can arrive before the client's
//! scene has finished loading, so an unmatched one waits a few seconds for its
//! entity before giving up and spawning fresh.

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use renzora_network_api::{
    ConnectionState, Delivery, NetOutbox, NetRequest, NetRole, NetworkId, NetworkOwner, NetworkStatus,
    NetworkTransform, Networked, OwnerKind, TransformAuthority,
};
use renzora_scripting_api::{IncomingRpc, ScriptRpcInbox};

use crate::interpolation::{NetClock, NetInterpolation};
use crate::protocol::{self, NetMessage, SpawnData, WireTarget, WireTransform, TRANSFORMS_PER_MESSAGE};
use crate::reflect_io;
use crate::transport::udp::Socket;
use crate::transport::{Channel, ClientEndpoint, ClientEvent, ClientState};

/// How long a spawn naming a scene entity waits for that entity to load.
const SCENE_MATCH_WAIT: Duration = Duration::from_secs(5);
/// Report owned transforms at most this often.
const MAX_OWNED_RATE: f64 = 30.0;
const OWNED_REFRESH_SECS: f64 = 1.0;

/// Marks an entity the network spawned, as opposed to one adopted from the
/// scene. On disconnect the first kind is despawned; the second is released
/// and stays, as it was before the session.
#[derive(Component)]
pub struct NetSpawned;

/// The live connection. Present from `connect` until the session ends.
#[derive(Resource)]
pub struct NetClient {
    endpoint: ClientEndpoint,
    socket: Socket,
    last_frame: Instant,
    client_id: Option<u64>,
    owned_interval: f64,
    next_owned: f64,
    owned_sent: HashMap<u64, (WireTransform, f64)>,
    by_net_id: HashMap<u64, Entity>,
    unmatched: Vec<(SpawnData, Instant)>,
}

/// Drain connect/disconnect requests. A process that is a server ignores
/// them; a client already connected is told so.
pub fn process_requests(world: &mut World) {
    if world.contains_resource::<crate::server::NetServer>() {
        return;
    }
    let requests = std::mem::take(&mut world.resource_mut::<NetOutbox>().requests);
    for request in requests {
        match request {
            NetRequest::Connect { address, port } => connect(world, address, port),
            NetRequest::Disconnect => {
                if let Some(mut client) = world.get_resource_mut::<NetClient>() {
                    client.endpoint.disconnect();
                }
            }
        }
    }
}

fn connect(world: &mut World, address: String, port: u16) {
    if world.contains_resource::<NetClient>() {
        warn!("[network] connect() ignored: already connected or connecting");
        return;
    }
    let fallback = world
        .get_resource::<renzora_project_api::CurrentProject>()
        .and_then(|p| p.config.network.clone())
        .unwrap_or_default();
    let address = if address.trim().is_empty() { fallback.server_addr } else { address };
    let port = if port == 0 { fallback.port } else { port };

    let Some(server) = resolve(&address, port) else {
        error!("[network] Cannot resolve server address `{address}:{port}`");
        world.resource_mut::<NetworkStatus>().last_error = Some(format!("cannot resolve {address}:{port}"));
        return;
    };
    let bind: SocketAddr = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().unwrap_or(server);
    let socket = match Socket::bind(bind) {
        Ok(s) => s,
        Err(e) => {
            error!("[network] Could not open a UDP socket: {e}");
            return;
        }
    };
    info!("[network] Connecting to {server}");
    let now = Instant::now();
    let protocol = crate::server::protocol_id(world);
    world.insert_resource(NetClient {
        endpoint: ClientEndpoint::new(server, protocol, now),
        socket,
        last_frame: now,
        client_id: None,
        owned_interval: 1.0 / MAX_OWNED_RATE,
        next_owned: 0.0,
        owned_sent: HashMap::new(),
        by_net_id: HashMap::new(),
        unmatched: Vec::new(),
    });
    world.insert_resource(NetClock::default());
    let mut status = world.resource_mut::<NetworkStatus>();
    status.role = NetRole::Client;
    status.is_server = false;
    status.state = ConnectionState::Connecting;
    status.last_error = None;
}

/// `host:port` to an address, preferring IPv4: a server bound to `0.0.0.0`
/// (what `--server` does) does not answer on `::1`, which is what `localhost`
/// resolves to first on Windows.
fn resolve(address: &str, port: u16) -> Option<SocketAddr> {
    let all: Vec<SocketAddr> = (address, port).to_socket_addrs().ok()?.collect();
    all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
}

/// Read the socket and apply what the server sent. First in the frame.
pub fn client_receive(world: &mut World) {
    let Some(mut client) = world.remove_resource::<NetClient>() else { return };
    let now = Instant::now();
    {
        let NetClient { socket, endpoint, .. } = &mut client;
        socket.drain(|from, bytes| endpoint.receive(from, bytes, now));
    }
    let dt = now.duration_since(client.last_frame).as_secs_f64();
    client.last_frame = now;
    world.resource_mut::<NetClock>().advance(dt);

    let mut ended = None;
    for event in client.endpoint.take_events() {
        match event {
            ClientEvent::Connected { client_id, tick_rate } => {
                info!("[network] Connected as player {client_id}");
                // In: a later drop is a real disconnect, not a slow start.
                world.remove_resource::<ConnectRetry>();
                client.client_id = Some(client_id);
                client.owned_interval = 1.0 / (tick_rate.max(1) as f64).min(MAX_OWNED_RATE);
                let mut status = world.resource_mut::<NetworkStatus>();
                status.state = ConnectionState::Connected;
                status.client_id = Some(client_id);
            }
            ClientEvent::Disconnected { reason } => ended = Some(reason),
            ClientEvent::Message(_, bytes) => {
                if let Some(msg) = protocol::decode(&bytes) {
                    handle(world, &mut client, msg, now);
                }
            }
        }
    }

    // Spawns still waiting for their scene entity.
    let waiting = std::mem::take(&mut client.unmatched);
    for (data, since) in waiting {
        let expired = now.duration_since(since) >= SCENE_MATCH_WAIT;
        if let Some(local) = data.scene_id.and_then(|id| find_scene_entity(world, id)) {
            adopt(world, &mut client, data, local);
        } else if expired {
            let e = world.spawn(NetSpawned).id();
            adopt(world, &mut client, data, e);
        } else {
            client.unmatched.push((data, since));
        }
    }

    match ended {
        Some(reason) => end_session(world, client, reason),
        None => world.insert_resource(client),
    }
}

fn handle(world: &mut World, client: &mut NetClient, msg: NetMessage, now: Instant) {
    match msg {
        NetMessage::Roster { players } => world.resource_mut::<NetworkStatus>().players = players,
        NetMessage::Spawn(data) => {
            if let Some(&existing) = client.by_net_id.get(&data.net_id) {
                adopt(world, client, data, existing);
            } else if let Some(scene_id) = data.scene_id {
                match find_scene_entity(world, scene_id) {
                    Some(local) => adopt(world, client, data, local),
                    None => client.unmatched.push((data, now)),
                }
            } else {
                let e = world.spawn(NetSpawned).id();
                adopt(world, client, data, e);
            }
        }
        NetMessage::Despawn { net_id } => {
            client.unmatched.retain(|(d, _)| d.net_id != net_id);
            if let Some(e) = client.by_net_id.remove(&net_id) {
                if let Ok(em) = world.get_entity_mut(e) {
                    em.despawn();
                }
            }
        }
        NetMessage::Update { net_id, changed, removed } => {
            if let Some(&e) = client.by_net_id.get(&net_id) {
                let registry = world.resource::<AppTypeRegistry>().clone();
                let registry = registry.read();
                for (path, bytes) in &changed {
                    reflect_io::write_component(world, &registry, e, path, bytes);
                }
                for path in &removed {
                    reflect_io::remove_component(world, &registry, e, path);
                }
                drop(registry);
                configure_motion(world, client.client_id, e);
            } else if let Some((data, _)) = client.unmatched.iter_mut().find(|(d, _)| d.net_id == net_id) {
                data.components.retain(|(p, _)| !removed.contains(p) && !changed.iter().any(|(c, _)| c == p));
                data.components.extend(changed);
            }
        }
        NetMessage::Snapshot { time, transforms } => {
            let rtt = client.endpoint.rtt() as f64;
            world.resource_mut::<NetClock>().observe(time, rtt);
            for (net_id, t) in transforms {
                let Some(&e) = client.by_net_id.get(&net_id) else { continue };
                if let Some(mut interp) = world.get_mut::<NetInterpolation>(e) {
                    interp.push(time, t);
                }
            }
        }
        NetMessage::Rpc { from, name, args, .. } => {
            world.resource_mut::<ScriptRpcInbox>().pending.push(IncomingRpc {
                name,
                args: protocol::args_from_bytes(&args),
                from,
            });
        }
        // Only a client sends these.
        NetMessage::Owned { .. } => {}
    }
}

fn find_scene_entity(world: &mut World, scene_id: u64) -> Option<Entity> {
    let mut q = world.query::<(Entity, &renzora_scene_api::SceneEntityId)>();
    q.iter(world).find(|(_, s)| s.0 == scene_id).map(|(e, _)| e)
}

/// Make `entity` the local copy of a replicated entity.
fn adopt(world: &mut World, client: &mut NetClient, data: SpawnData, entity: Entity) {
    client.by_net_id.insert(data.net_id, entity);
    {
        let Ok(mut em) = world.get_entity_mut(entity) else { return };
        let mut transform = em.get::<Transform>().copied().unwrap_or_default();
        data.transform.apply(&mut transform);
        em.insert((transform, NetworkId(data.net_id), Networked));
        if !em.contains::<Visibility>() {
            em.insert(Visibility::default());
        }
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    for (path, bytes) in &data.components {
        reflect_io::write_component(world, &registry, entity, path, bytes);
    }
    drop(registry);
    configure_motion(world, client.client_id, entity);
    let clock = world.resource::<NetClock>();
    let (synced, now) = (clock.synced, clock.now);
    if synced {
        if let Some(mut interp) = world.get_mut::<NetInterpolation>(entity) {
            interp.push(now, data.transform);
        }
    }
}

/// Decide who moves `entity` here: this client, for its own owner-moved
/// avatar, and the network (through interpolation) for everything else.
fn configure_motion(world: &mut World, me: Option<u64>, entity: Entity) {
    let Ok(mut em) = world.get_entity_mut(entity) else { return };
    let settings = em.get::<NetworkTransform>().cloned().unwrap_or_default();
    let owner = em.get::<NetworkOwner>().map(|o| o.0).unwrap_or_default();
    let mine = settings.authority == TransformAuthority::Owner
        && me.is_some()
        && owner == OwnerKind::Client(me.unwrap_or_default());
    if mine {
        em.remove::<NetInterpolation>();
    } else {
        match em.get_mut::<NetInterpolation>() {
            Some(mut interp) => interp.smooth = settings.interpolate,
            None => {
                em.insert(NetInterpolation::new(settings.interpolate));
            }
        }
    }
}

/// Tear down after the connection ended, however it ended.
fn end_session(world: &mut World, client: NetClient, reason: String) {
    if client.client_id.is_none() && retry_connect(world, &reason) {
        world.resource_mut::<NetworkStatus>().state = ConnectionState::Disconnected;
        return;
    }
    info!("[network] Disconnected: {reason}");
    for (_, e) in client.by_net_id {
        let Ok(mut em) = world.get_entity_mut(e) else { continue };
        if em.contains::<NetSpawned>() {
            em.despawn();
        } else {
            // Adopted from the scene: release it, as it was before.
            em.remove::<(NetworkId, NetInterpolation)>();
        }
    }
    world.remove_resource::<NetClock>();
    let mut status = world.resource_mut::<NetworkStatus>();
    let by_choice = reason == "disconnected";
    *status = NetworkStatus {
        last_error: (!by_choice).then_some(reason),
        ..Default::default()
    };
}

/// Send RPCs and owned transforms, and flush the socket. Last in the frame.
pub fn client_send(world: &mut World) {
    let Some(mut client) = world.remove_resource::<NetClient>() else { return };
    let connected = matches!(client.endpoint.state(), ClientState::Connected { .. });

    // RPCs wait in the outbox while connecting, and go once we are in.
    if connected {
        let rpcs = std::mem::take(&mut world.resource_mut::<NetOutbox>().rpcs);
        let me = client.client_id.unwrap_or_default();
        for rpc in rpcs {
            let channel = match rpc.delivery {
                Delivery::Reliable => Channel::Reliable,
                Delivery::Unreliable => Channel::Unreliable,
            };
            let msg = NetMessage::Rpc {
                from: me,
                target: WireTarget::from(rpc.target),
                name: rpc.name,
                args: protocol::args_to_bytes(&rpc.args),
            };
            client.endpoint.send(channel, protocol::encode(&msg));
        }

        let clock = world.resource::<NetClock>().now;
        if clock >= client.next_owned {
            client.next_owned = clock + client.owned_interval;
            send_owned(world, &mut client, clock);
        }
    }

    let now = Instant::now();
    client.endpoint.update(now);
    for bytes in client.endpoint.take_outgoing() {
        client.socket.send(client.endpoint.server(), &bytes);
    }

    let stats = client.endpoint.stats();
    {
        let mut status = world.resource_mut::<NetworkStatus>();
        status.rtt_ms = stats.rtt_ms;
        status.jitter_ms = stats.jitter_ms;
        status.packet_loss = stats.packet_loss;
        status.bytes_in_per_sec = stats.bytes_in_per_sec;
        status.bytes_out_per_sec = stats.bytes_out_per_sec;
        status.replicated_entities = client.by_net_id.len();
    }
    // A timeout or refusal shows up as an event on the next receive, which
    // then ends the session; here only the bookkeeping is put back.
    world.insert_resource(client);
}

fn send_owned(world: &mut World, client: &mut NetClient, clock: f64) {
    let Some(me) = client.client_id else { return };
    let mut reports = Vec::new();
    for (&net_id, &entity) in &client.by_net_id {
        let Ok(er) = world.get_entity(entity) else { continue };
        let settings = er.get::<NetworkTransform>().cloned().unwrap_or_default();
        let owner = er.get::<NetworkOwner>().map(|o| o.0).unwrap_or_default();
        if settings.authority != TransformAuthority::Owner || owner != OwnerKind::Client(me) {
            continue;
        }
        let Some(t) = er.get::<Transform>() else { continue };
        let wire = WireTransform::from_transform(t, settings.sync_rotation, settings.sync_scale);
        let due = client
            .owned_sent
            .get(&net_id)
            .is_none_or(|(last, at)| last.differs(&wire) || clock - at >= OWNED_REFRESH_SECS);
        if due {
            client.owned_sent.insert(net_id, (wire, clock));
            reports.push((net_id, wire));
        }
    }
    for chunk in reports.chunks(TRANSFORMS_PER_MESSAGE) {
        let msg = NetMessage::Owned { time: clock, transforms: chunk.to_vec() };
        client.endpoint.send(Channel::Unreliable, protocol::encode(&msg));
    }
}

/// Leave cleanly when the app exits, so the server frees the slot at once.
pub fn client_shutdown(mut exit: MessageReader<AppExit>, client: Option<ResMut<NetClient>>) {
    if exit.read().next().is_none() {
        return;
    }
    let Some(mut client) = client else { return };
    client.endpoint.disconnect();
    for bytes in client.endpoint.take_outgoing() {
        client.socket.send(client.endpoint.server(), &bytes);
    }
}

/// `--connect [address[:port]]`: join a server at startup. Bare `--connect`
/// uses the project's `[network]` address. This is how Play starts its extra
/// test clients, and how a player can be pointed at a server from a shortcut.
pub fn connect_from_args(mut commands: Commands, mut outbox: ResMut<NetOutbox>) {
    if let Some((address, port)) = connect_arg(std::env::args().collect()) {
        outbox.requests.push(NetRequest::Connect { address: address.clone(), port });
        commands.insert_resource(ConnectRetry {
            address,
            port,
            until: Instant::now() + CONNECT_RETRY_FOR,
        });
    }
}

/// How long a `--connect` launch keeps trying a server that does not answer.
const CONNECT_RETRY_FOR: Duration = Duration::from_secs(60);

/// A `--connect` launch tries again when the server does not answer, until
/// [`CONNECT_RETRY_FOR`] has passed. Play starts the host and its test clients
/// at the same moment, and a cold host can take longer to open its port than a
/// client's handshake waits.
#[derive(Resource)]
struct ConnectRetry {
    address: String,
    port: u16,
    until: Instant,
}

/// Called when a connection attempt ended. True if it was retried.
fn retry_connect(world: &mut World, reason: &str) -> bool {
    let Some(retry) = world.get_resource::<ConnectRetry>() else { return false };
    if reason != crate::transport::endpoint::NO_ANSWER || Instant::now() >= retry.until {
        world.remove_resource::<ConnectRetry>();
        return false;
    }
    let request = NetRequest::Connect { address: retry.address.clone(), port: retry.port };
    world.resource_mut::<NetOutbox>().requests.push(request);
    info!("[network] No answer yet, trying again");
    true
}

fn connect_arg(args: Vec<String>) -> Option<(String, u16)> {
    let i = args.iter().position(|a| a == "--connect")?;
    let target = args.get(i + 1).filter(|v| !v.starts_with("--")).cloned().unwrap_or_default();
    match target.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && !host.ends_with(':') => {
            Some((host.to_string(), port.parse().unwrap_or(0)))
        }
        _ => Some((target, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::connect_arg;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn connect_flag_forms() {
        assert_eq!(connect_arg(args("game")), None);
        assert_eq!(connect_arg(args("game --connect")), Some((String::new(), 0)));
        assert_eq!(connect_arg(args("game --connect --project x")), Some((String::new(), 0)));
        assert_eq!(connect_arg(args("game --connect 10.0.0.2")), Some(("10.0.0.2".into(), 0)));
        assert_eq!(connect_arg(args("game --connect play.example.net:9000")), Some(("play.example.net".into(), 9000)));
    }
}
