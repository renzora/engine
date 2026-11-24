//! Multiplayer: a from-scratch UDP transport, server-authoritative
//! replication, RPCs and player avatars.
//!
//! What a game uses is in the contract, `renzora_network_api`
//! (`renzora::network` to a script). This crate is what makes it happen, and
//! is an optional engine feature: a single-player export strips it.
//!
//! | Module | Job |
//! |---|---|
//! | [`transport`] | datagrams, handshake, reliability, fragmentation (sans-IO) |
//! | [`protocol`] | the game messages carried over it |
//! | [`server`] | accept players, route RPCs, replicate |
//! | [`client`] | connect, mirror the server's world, report own avatar |
//! | [`interpolation`] | smooth remote motion on a shared server clock |
//! | [`spawner`] | one avatar per player, prefab instancing |
//! | [`reflect_io`] | components to bytes by type path |
//! | [`script_extension`] | the `rpc()` / `net_*` verbs of language plugins |
//!
//! Native only. A browser cannot open a UDP socket, so on wasm the plugin
//! registers the types and nothing else, and every `renzora::network` call
//! behaves as it does offline.

pub mod components;
pub mod config;
pub mod status;

#[cfg(not(target_arch = "wasm32"))]
pub mod client;
#[cfg(not(target_arch = "wasm32"))]
pub mod interpolation;
#[cfg(not(target_arch = "wasm32"))]
pub mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod reflect_io;
#[cfg(not(target_arch = "wasm32"))]
pub mod rpc;
#[cfg(not(target_arch = "wasm32"))]
pub mod script_extension;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
#[cfg(not(target_arch = "wasm32"))]
pub mod spawner;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;

pub use components::*;
pub use config::{NetworkConfig, TransportKind};
#[cfg(not(target_arch = "wasm32"))]
pub use server::NetworkServerPlugin;
pub use status::NetworkStatus;

use bevy::prelude::*;
use renzora_network_api::{NetOutbox, ReplicationRegistry};

/// The network runtime. On every peer; the server half is added beside it by
/// [`NetworkServerPlugin`] for `--server` and `--host`.
#[derive(Default)]
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        info!("[network] NetworkPlugin");

        app.init_resource::<NetworkStatus>()
            .init_resource::<NetOutbox>()
            .init_resource::<renzora_scripting_api::NetworkBridge>()
            .init_resource::<renzora_scripting_api::ScriptRpcInbox>()
            .init_resource::<renzora_scripting_api::ScriptNetLifecycleInbox>();
        // `init`, not `insert`: a plugin that built first may already have
        // registered its own components with `app.replicate::<T>()`.
        if !app.world().contains_resource::<ReplicationRegistry>() {
            app.init_resource::<ReplicationRegistry>();
        }

        app.register_type::<Networked>()
            .register_type::<NetworkId>()
            .register_type::<NetworkOwner>()
            .register_type::<OwnerKind>()
            .register_type::<NetworkTransform>()
            .register_type::<TransformAuthority>()
            .register_type::<NetworkPlayer>()
            .register_type::<NetworkPrefab>()
            .register_type::<NetworkPlayerSpawner>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            app.init_resource::<spawner::PendingAvatars>();
            app.add_observer(script_extension::handle_network_script_actions);
            app.add_systems(Startup, client::connect_from_args);
            app.add_systems(PreUpdate, (server::server_receive, client::client_receive));
            app.add_systems(
                Update,
                (
                    client::process_requests,
                    rpc::route_offline,
                    spawner::spawn_avatars,
                    spawner::instance_prefabs,
                    interpolation::apply_interpolation.run_if(resource_exists::<interpolation::NetClock>),
                    sync_network_bridge,
                ),
            );
            app.add_systems(
                PostUpdate,
                (server::server_send, client::client_send, server::server_shutdown, client::client_shutdown),
            );
        }
    }
}

/// Mirror the status into the small `NetworkBridge` the scripting context
/// reads, which cannot depend on this crate.
fn sync_network_bridge(status: Res<NetworkStatus>, mut bridge: ResMut<renzora_scripting_api::NetworkBridge>) {
    if !status.is_changed() {
        return;
    }
    bridge.is_server = status.is_server;
    bridge.is_connected = status.is_connected();
    bridge.player_count = status.players.len().max(status.connected_clients.len()) as i32;
}
