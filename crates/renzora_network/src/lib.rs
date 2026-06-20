//! Renzora networking crate — from-scratch UDP multiplayer (client/server/host).
//!
//! On native platforms: a synchronous UDP transport (see [`transport`]) with a
//! reliable RPC channel + connection lifecycle. On WASM: no-op stub (UDP
//! sockets are unavailable).

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
#[cfg(not(target_arch = "wasm32"))]
pub mod rpc;
#[cfg(not(target_arch = "wasm32"))]
pub mod script_extension;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod status;
#[cfg(not(target_arch = "wasm32"))]
mod transport;

pub use components::{
    NetworkId, NetworkOwner, NetworkPlayer, NetworkTransform, Networked, OwnerKind,
};
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
#[derive(Default)]
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        info!("[network] NetworkPlugin");

        // Shared status resource (read by editor panels, scripts, blueprints)
        app.init_resource::<NetworkStatus>();
        app.init_resource::<renzora::NetworkBridge>();
        // RPC bridge resources — outbound queue (drained to the wire) and the
        // inbound inbox (drained by renzora_scripting into `on_rpc`). Init'd
        // here so both the client and dedicated-server paths share them.
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.init_resource::<rpc::PendingOutgoingRpc>();
            app.init_resource::<renzora::ScriptRpcInbox>();
            app.init_resource::<renzora::ScriptNetLifecycleInbox>();
        }

        // Register networked component types (scene serialization + inspector).
        app.register_type::<components::Networked>();
        app.register_type::<components::NetworkId>();
        app.register_type::<components::NetworkOwner>();
        app.register_type::<components::OwnerKind>();
        app.register_type::<components::NetworkTransform>();
        app.register_type::<components::NetworkPlayer>();

        // A dedicated server adds `ServerPlugins` + protocol via
        // `NetworkServerPlugin`; skip the client setup here so the protocol
        // isn't registered twice.
        if app.world().contains_resource::<renzora::DedicatedServer>() {
            info!("[network] Dedicated server mode — skipping client setup");
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Host/listen-server mode (`--host`) runs BOTH plugin sets in one
            // process. There, `NetworkServerPlugin` (added after this) owns the
            // protocol, the shared observers, and the RPC-send system so each
            // registers exactly once, after `ServerPlugins`. Here we add only
            // the client half; in pure-client mode we add everything.
            // Host/listen-server (`--host`) runs BOTH plugin sets in one
            // process. The host IS the server (its scripts run server-side), so
            // it needs no separate local client link — `NetworkServerPlugin`
            // (added after this) owns the script-action observer + RPC send.
            let host = app.world().contains_resource::<renzora::HostServer>();

            // Client connection lifecycle + status. `process_pending_connect`
            // builds a `NetworkClient` from a `PendingNetworkConnect` request;
            // `client_poll` pumps it and delivers received RPCs to scripts.
            app.add_systems(
                Update,
                (
                    process_pending_connect,
                    process_pending_disconnect,
                    client::client_poll,
                    client::update_network_status,
                    sync_network_bridge,
                ),
            );

            if !host {
                // Pure client: own the script-action observer + RPC send.
                app.add_observer(script_extension::handle_network_script_actions);
                app.add_systems(Update, rpc::send_outgoing_rpcs);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            info!("[network] Networking disabled on WASM");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_pending_connect(
    mut commands: Commands,
    pending: Option<Res<PendingNetworkConnect>>,
    status: Option<Res<NetworkStatus>>,
) {
    let Some(pending) = pending else {
        return;
    };
    if status.is_some_and(|s| s.is_connected()) {
        log::warn!("[network] Already connected — ignoring connect request");
        commands.remove_resource::<PendingNetworkConnect>();
        return;
    }

    let server_addr = format!("{}:{}", pending.address, pending.port)
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| {
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                pending.port,
            )
        });
    commands.remove_resource::<PendingNetworkConnect>();

    info!("[network] Connecting to {} ...", server_addr);
    match client::NetworkClient::connect(server_addr, client::rand_client_id()) {
        // Status flips to Connecting → Connected via `update_network_status`.
        Ok(client) => commands.insert_resource(client),
        Err(e) => log::error!("[network] Failed to open client socket: {e}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_pending_disconnect(
    mut commands: Commands,
    pending: Option<Res<PendingNetworkDisconnect>>,
    mut client: Option<ResMut<client::NetworkClient>>,
) {
    if pending.is_none() {
        return;
    }
    if let Some(client) = client.as_mut() {
        info!("[network] Disconnecting");
        client.disconnect();
    }
    commands.remove_resource::<client::NetworkClient>();
    commands.remove_resource::<PendingNetworkDisconnect>();
    // `update_network_status` flips the status to Disconnected next frame.
}

/// Sync the lightweight `NetworkBridge` resource from the full `NetworkStatus`.
#[cfg(not(target_arch = "wasm32"))]
fn sync_network_bridge(status: Res<NetworkStatus>, mut bridge: ResMut<renzora::NetworkBridge>) {
    bridge.is_server = status.is_server;
    bridge.is_connected = status.is_connected();
    bridge.player_count = status.connected_clients.len() as i32;
}

renzora::add!(NetworkPlugin);
