//! Server-side networking plugin and systems.

use bevy::prelude::*;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

use crate::components::*;
use crate::config::NetworkConfig;
use crate::protocol;
use crate::status::{ConnectedClient, ConnectionState, NetworkStatus};

use core::time::Duration;

/// Server-side networking plugin.
///
/// Adds Lightyear server plugins, auto-replication, connection handling,
/// and spawns the server entity on startup.
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

        let tick_duration = Duration::from_secs_f64(1.0 / self.config.tick_rate as f64);

        app.add_plugins(ServerPlugins { tick_duration });

        // Register protocol (must be after ServerPlugins)
        protocol::register_protocol(app);

        // Server status
        let status = NetworkStatus {
            is_server: true,
            state: ConnectionState::Connected,
            ..Default::default()
        };
        app.insert_resource(status);

        // Store config for startup system
        app.insert_resource(ServerNetworkConfig(self.config.clone()));

        // RPC: scripts running on the server can send (broadcast to all
        // clients), and the server relays client→client RPCs while also
        // delivering them to its own scripts. The outbound queue + inbox are
        // init'd by `NetworkPlugin` (which builds before this in both paths).
        // `NetworkPlugin` skips its own observer setup in server mode, so the
        // script-action observer is added here for server-side scripts.
        app.add_observer(crate::script_extension::handle_network_script_actions);
        app.add_observer(crate::rpc::receive_and_relay_rpcs);

        // Systems
        app.add_systems(Startup, spawn_server_entity);
        app.add_systems(
            Update,
            (
                auto_replicate_networked,
                handle_new_clients,
                assign_network_ids,
                crate::rpc::send_outgoing_rpcs,
            ),
        );

        // Host/listen-server: this process also plays. Once the server has
        // started, spawn a local client linked to it; lightyear's host
        // observers promote it to a `HostClient` (in-process, no UDP for the
        // local player). The client half (`ClientPlugins`) was added by
        // `NetworkPlugin`, which ran first because it builds during
        // `add_engine_plugins` and this plugin is added afterwards.
        if app.world().contains_resource::<renzora::HostServer>() {
            info!("[network] Host mode — local player will join as a host client");
            app.add_systems(Update, spawn_host_client);
        }
    }
}

/// Resource holding the server config for the startup system.
#[derive(Resource)]
struct ServerNetworkConfig(NetworkConfig);

/// The server entity spawned at startup. Used by host mode to link the local
/// client to it. Harmless in dedicated-server mode (no system reads it).
#[derive(Resource)]
struct ServerEntity(Entity);

/// Spawn the server entity with UDP IO and netcode, then trigger Start.
fn spawn_server_entity(mut commands: Commands, config: Res<ServerNetworkConfig>) {
    let addr = format!("0.0.0.0:{}", config.0.port)
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| {
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                config.0.port,
            )
        });

    info!("[network] Spawning server entity on {}", addr);

    let server_entity = commands
        .spawn((
            ServerUdpIo::default(),
            LocalAddr(addr),
            NetcodeServer::new(NetcodeConfig::default()),
            Name::new("NetworkServer"),
        ))
        .id();

    commands.insert_resource(ServerEntity(server_entity));

    // Trigger the server to start listening
    commands.trigger(Start {
        entity: server_entity,
    });
}

/// Host mode only: once the server has had a few ticks to reach `Started`,
/// spawn the local player's client as a `LinkOf` the server and connect it.
/// Lightyear's host observers then promote it to a `HostClient`. Runs once.
///
/// The small frame delay mirrors the validated host-server recipe
/// (`tests/host_server.rs`): the server must be started before the local
/// client connects.
fn spawn_host_client(
    mut commands: Commands,
    server: Option<Res<ServerEntity>>,
    mut frames: Local<u32>,
    mut done: Local<bool>,
) {
    use lightyear::prelude::{Client, Connect, LinkOf};

    if *done {
        return;
    }
    let Some(server) = server else {
        return;
    };
    *frames += 1;
    if *frames < 5 {
        return;
    }

    info!(
        "[network] Host mode: joining as local host client (server {:?})",
        server.0
    );
    let client = commands
        .spawn((
            Client::default(),
            LinkOf { server: server.0 },
            Name::new("HostClient"),
        ))
        .id();
    commands.trigger(Connect { entity: client });
    *done = true;
}

/// When an entity gains the `Networked` marker, set it up for replication:
/// replicate to all clients, and (unless `NetworkTransform.interpolate` is
/// false) mark it as an interpolation target so remote peers smooth its
/// `Transform` between snapshots. `Transform` itself replicates because it's
/// registered in the protocol.
fn auto_replicate_networked(
    mut commands: Commands,
    query: Query<(Entity, Option<&NetworkTransform>), Added<Networked>>,
) {
    for (entity, net_tf) in &query {
        let interpolate = net_tf.is_none_or(|nt| nt.interpolate);
        let mut ec = commands.entity(entity);
        ec.insert(Replicate::to_clients(NetworkTarget::All));
        if interpolate {
            ec.insert(InterpolationTarget::to_clients(NetworkTarget::All));
        }
        info!("[network] Auto-replicate entity {:?} (interpolate={})", entity, interpolate);
    }
}

/// Assign `NetworkId` to newly networked entities that don't have one yet.
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

/// Track new client connections by observing `Added<Connected>`.
fn handle_new_clients(
    query: Query<Entity, Added<Connected>>,
    mut status: ResMut<NetworkStatus>,
    mut next_client_id: Local<u64>,
) {
    for entity in &query {
        *next_client_id += 1;
        info!(
            "[network] Client connected: entity={:?} id={}",
            entity, *next_client_id
        );
        status.connected_clients.push(ConnectedClient {
            client_id: *next_client_id,
            rtt_ms: 0.0,
        });
    }
}
