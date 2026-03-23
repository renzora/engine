//! Server-side networking plugin and systems.

use bevy::prelude::*;
use lightyear::prelude::*;
use lightyear::prelude::server::*;

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

        app.add_plugins(ServerPlugins {
            tick_duration,
        });

        // Register protocol (must be after ServerPlugins)
        protocol::register_protocol(app);

        // Server status
        let mut status = NetworkStatus::default();
        status.is_server = true;
        status.state = ConnectionState::Connected;
        app.insert_resource(status);

        // Store config for startup system
        app.insert_resource(ServerNetworkConfig(self.config.clone()));

        // Systems
        app.add_systems(Startup, spawn_server_entity);
        app.add_systems(Update, (
            auto_replicate_networked,
            handle_new_clients,
            assign_network_ids,
        ));
    }
}

/// Resource holding the server config for the startup system.
#[derive(Resource)]
struct ServerNetworkConfig(NetworkConfig);

/// Spawn the server entity with UDP IO and netcode, then trigger Start.
fn spawn_server_entity(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
) {
    let addr = format!("0.0.0.0:{}", config.0.port)
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| {
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                config.0.port,
            )
        });

    info!("[network] Spawning server entity on {}", addr);

    let server_entity = commands.spawn((
        ServerUdpIo::default(),
        LocalAddr(addr),
        NetcodeServer::new(NetcodeConfig::default()),
        Name::new("NetworkServer"),
    )).id();

    // Trigger the server to start listening
    commands.trigger(Start { entity: server_entity });
}

/// Auto-insert Lightyear `Replicate` marker on entities that gain the `Networked` marker.
fn auto_replicate_networked(
    mut commands: Commands,
    query: Query<Entity, Added<Networked>>,
) {
    for entity in &query {
        commands.entity(entity).insert(Replicate::default());
        info!("[network] Auto-replicate entity {:?}", entity);
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
        info!("[network] Client connected: entity={:?} id={}", entity, *next_client_id);
        status.connected_clients.push(ConnectedClient {
            client_id: *next_client_id,
            rtt_ms: 0.0,
        });
    }
}
