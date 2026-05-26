//! Host-server (listen-server) recipe validation — pure lightyear, no renzora,
//! so it links and runs on native Windows.
//!
//! Proves the two things that would otherwise have to be discovered via a slow
//! Docker rebuild:
//!   1. one `App` can hold BOTH `ClientPlugins` and `ServerPlugins` without a
//!      duplicate-plugin panic at build, and
//!   2. a local `Client` that is a `LinkOf` a started `Server`, once `Connect`
//!      is triggered, gets promoted to a `HostClient` by lightyear's observers.
//!
//! If this is green, the same setup can be wired into `renzora_network`'s
//! `--host` mode with confidence.

use bevy::prelude::*;
use core::time::Duration;
use std::net::SocketAddr;

use lightyear::connection::host::HostClient;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::{NetcodeConfig, NetcodeServer, ServerPlugins, ServerUdpIo, Start};
use lightyear::prelude::{Client, Connect, LinkOf, LocalAddr};

fn host_server_app() -> App {
    let tick = Duration::from_secs_f64(1.0 / 64.0);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // The make-or-break line: both plugin groups in one app. If they double-add
    // a shared sub-plugin this panics here.
    app.add_plugins(ClientPlugins { tick_duration: tick });
    app.add_plugins(ServerPlugins { tick_duration: tick });
    app
}

#[test]
fn both_plugin_sets_coexist() {
    // Just building the app is the assertion — no panic on duplicate plugins.
    let _app = host_server_app();
}

#[test]
fn local_client_is_promoted_to_host_client() {
    let mut app = host_server_app();

    // Spawn + start the server. Ephemeral UDP port; the host client never uses
    // it (it connects in-process), but the server must reach `Started`.
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = app
        .world_mut()
        .spawn((
            ServerUdpIo::default(),
            LocalAddr(addr),
            NetcodeServer::new(NetcodeConfig::default()),
        ))
        .id();
    app.world_mut().trigger(Start { entity: server });
    for _ in 0..10 {
        app.update();
    }

    // Spawn the local client as a LinkOf the server, then connect it.
    let client = app
        .world_mut()
        .spawn((Client::default(), LinkOf { server }))
        .id();
    app.world_mut().trigger(Connect { entity: client });
    for _ in 0..10 {
        app.update();
    }

    assert!(
        app.world().get::<HostClient>(client).is_some(),
        "local client was not promoted to HostClient in host-server mode"
    );
}
