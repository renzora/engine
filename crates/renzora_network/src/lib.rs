//! Renzora networking crate — Lightyear-based multiplayer for dedicated server architecture.
//!
//! On native platforms: full Lightyear UDP networking with client/server.
//! On WASM: no-op stub (UDP sockets not available).

#[cfg(not(target_arch = "wasm32"))]
pub mod client;
pub mod components;
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod input;
#[cfg(not(target_arch = "wasm32"))]
pub mod messages;
#[cfg(not(target_arch = "wasm32"))]
pub mod prediction;
#[cfg(not(target_arch = "wasm32"))]
pub mod protocol;
pub mod script_extension;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod status;

pub use components::{NetworkId, NetworkOwner, Networked, OwnerKind};
pub use config::{NetworkConfig, TransportKind};
#[cfg(not(target_arch = "wasm32"))]
pub use input::PlayerInput;
#[cfg(not(target_arch = "wasm32"))]
pub use messages::{ChatMessage, DespawnRequest, GameEvent, SpawnRequest};
#[cfg(not(target_arch = "wasm32"))]
pub use server::NetworkServerPlugin;
pub use status::NetworkStatus;

use bevy::prelude::*;

// ── Dynamic connect/disconnect resources ────────────────────────────────────

/// Insert this resource to request a client connection to a server.
/// Consumed by `process_pending_connect`.
#[derive(Resource)]
pub struct PendingNetworkConnect {
    pub address: String,
    pub port: u16,
}

/// Insert this resource to request disconnection from the server.
/// Consumed by `process_pending_disconnect`.
#[derive(Resource)]
pub struct PendingNetworkDisconnect;

/// Tracks the client entity spawned by dynamic connect, so we can disconnect it.
#[derive(Resource)]
pub struct ActiveClientEntity(pub Entity);

/// Runtime networking plugin.
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        info!("[network] NetworkPlugin");

        // Shared status resource (read by editor panels, scripts, blueprints)
        app.init_resource::<NetworkStatus>();
        app.init_resource::<renzora::NetworkBridge>();

        // Register networked component types for scene deny list
        app.register_type::<components::Networked>();
        app.register_type::<components::NetworkId>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Lightyear client infrastructure
            let tick_rate = app
                .world()
                .get_resource::<renzora::CurrentProject>()
                .and_then(|p| p.config.network.as_ref())
                .map(|n| n.tick_rate)
                .unwrap_or(64);
            let tick_duration = core::time::Duration::from_secs_f64(1.0 / tick_rate as f64);
            app.add_plugins(lightyear::prelude::client::ClientPlugins { tick_duration });

            // Register protocol (channels, components, messages)
            protocol::register_protocol(app);

            // Script actions (decoupled — observes ScriptAction events)
            app.add_observer(script_extension::handle_network_script_actions);

            // Schedule systems
            app.add_systems(
                Update,
                (
                    process_pending_connect,
                    process_pending_disconnect,
                    client::update_network_status,
                    sync_network_bridge,
                ),
            );
        }

        #[cfg(target_arch = "wasm32")]
        {
            info!("[network] Networking disabled on WASM");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_pending_connect(world: &mut World) {
    use lightyear::prelude::{Authentication, LocalAddr, UdpIo};
    use lightyear::prelude::client::{Connect, NetcodeClient, NetcodeConfig};

    let Some(pending) = world.remove_resource::<PendingNetworkConnect>() else {
        return;
    };

    if let Some(status) = world.get_resource::<NetworkStatus>() {
        if status.is_connected() {
            log::warn!("[network] Already connected — ignoring connect request");
            return;
        }
    }

    let server_addr = format!("{}:{}", pending.address, pending.port)
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| {
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                pending.port,
            )
        });

    info!("[network] Connecting to {} ...", server_addr);

    let auth = Authentication::Manual {
        server_addr,
        client_id: client::rand_client_id(),
        private_key: [0u8; 32],
        protocol_id: 0,
    };

    let netcode_client = match NetcodeClient::new(auth, NetcodeConfig::default()) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[network] Failed to create netcode client: {}", e);
            return;
        }
    };

    let local_addr: std::net::SocketAddr = "0.0.0.0:0".parse().unwrap();

    let client_entity = world
        .spawn((
            UdpIo::default(),
            LocalAddr(local_addr),
            netcode_client,
            Name::new("NetworkClient"),
            renzora::HideInHierarchy,
        ))
        .id();

    world.insert_resource(ActiveClientEntity(client_entity));
    world.trigger(Connect {
        entity: client_entity,
    });

    if let Some(mut status) = world.get_resource_mut::<NetworkStatus>() {
        status.state = status::ConnectionState::Connecting;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_pending_disconnect(world: &mut World) {
    use lightyear::prelude::client::Disconnect;

    let Some(_) = world.remove_resource::<PendingNetworkDisconnect>() else {
        return;
    };

    if let Some(active) = world.remove_resource::<ActiveClientEntity>() {
        info!("[network] Disconnecting client entity {:?}", active.0);
        if world.get_entity(active.0).is_ok() {
            world.trigger(Disconnect {
                entity: active.0,
            });
        }
    }

    if let Some(mut status) = world.get_resource_mut::<NetworkStatus>() {
        status.state = status::ConnectionState::Disconnected;
    }
}

/// Sync the lightweight `NetworkBridge` resource from the full `NetworkStatus`.
#[cfg(not(target_arch = "wasm32"))]
fn sync_network_bridge(
    status: Res<NetworkStatus>,
    mut bridge: ResMut<renzora::NetworkBridge>,
) {
    bridge.is_server = status.is_server;
    bridge.is_connected = status.is_connected();
    bridge.player_count = status.connected_clients.len() as i32;
}
