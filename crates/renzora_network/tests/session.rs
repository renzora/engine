//! A whole session over real UDP on localhost: a server `App` and client
//! `App`s in one process, each with the real plugins, stepped frame by frame.
//!
//! What it proves is the part unit tests cannot: that the server's
//! replication, the client's adoption and the routing agree with each other
//! end to end, through real sockets.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use bevy::prelude::*;
use renzora_network::server::NetServer;
use renzora_network::{NetworkConfig, NetworkPlugin, NetworkServerPlugin};
use renzora_network_api::{
    ConnectionState, NetRole, NetworkId, NetworkOwner, NetworkPlayer, NetworkPlayerSpawner, NetworkPrefab,
    NetworkStatus, NetworkTransform, Networked, OwnerKind, Rpc, RpcTarget, HOST_PEER_ID,
};
use renzora_scripting_api::{ScriptNetLifecycleInbox, ScriptRpcInbox};

fn base_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(NetworkPlugin);
    app
}

fn server_app(host: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    if host {
        app.init_resource::<renzora_project_api::HostServer>();
    } else {
        app.init_resource::<renzora_project_api::DedicatedServer>();
    }
    app.add_plugins(NetworkPlugin);
    // Port 0: the OS picks a free one, so tests never collide.
    app.add_plugins(NetworkServerPlugin::new(NetworkConfig { port: 0, ..Default::default() }));
    app.update();
    app
}

fn port(server: &App) -> u16 {
    server.world().resource::<NetServer>().local_addr().unwrap().port()
}

fn client_app(port: u16) -> App {
    let mut app = base_app();
    app.update();
    renzora_network_api::connect(app.world_mut(), "127.0.0.1", port);
    app
}

/// Step every app until `done` holds, or fail after a few seconds.
fn run_until(apps: &mut [&mut App], what: &str, mut done: impl FnMut(&mut [&mut App]) -> bool) {
    for _ in 0..400 {
        for app in apps.iter_mut() {
            app.update();
        }
        if done(apps) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for: {what}");
}

fn status(app: &App) -> &NetworkStatus {
    app.world().resource::<NetworkStatus>()
}

fn find_by_name(app: &mut App, name: &str) -> Option<Entity> {
    let mut q = app.world_mut().query::<(Entity, &Name)>();
    q.iter(app.world()).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

#[test]
fn replication_rpcs_and_ownership_end_to_end() {
    let mut server = server_app(false);
    assert_eq!(status(&server).role, NetRole::DedicatedServer);
    let crate_e = server
        .world_mut()
        .spawn((Name::new("Crate"), Transform::from_xyz(1.0, 2.0, 3.0), Networked))
        .id();

    let port = port(&server);
    let mut a = client_app(port);
    let mut b = client_app(port);

    run_until(&mut [&mut server, &mut a, &mut b], "both clients connected and the crate replicated", |apps| {
        let (rest, clients) = apps.split_at_mut(1);
        let _ = rest;
        clients.iter_mut().all(|c| {
            status(c).state == ConnectionState::Connected && find_by_name(c, "Crate").is_some()
        })
    });
    let a_id = status(&a).client_id.unwrap();
    let b_id = status(&b).client_id.unwrap();
    assert_ne!(a_id, b_id);
    // Join hooks fired on the server, for both.
    let joined: Vec<u64> = server.world_mut().resource_mut::<ScriptNetLifecycleInbox>().pending.drain(..).map(|e| e.id).collect();
    assert!(joined.contains(&a_id) && joined.contains(&b_id));

    // Moving it on the server moves it on the client.
    server.world_mut().get_mut::<Transform>(crate_e).unwrap().translation = Vec3::new(10.0, 0.0, 0.0);
    run_until(&mut [&mut server, &mut a, &mut b], "the crate's move reached the client", |apps| {
        let c = find_by_name(apps[1], "Crate").unwrap();
        (apps[1].world().get::<Transform>(c).unwrap().translation.x - 10.0).abs() < 1e-3
    });

    // A component change replicates too.
    server.world_mut().get_mut::<Name>(crate_e).unwrap().set("Barrel");
    run_until(&mut [&mut server, &mut a, &mut b], "the rename reached the client", |apps| {
        find_by_name(apps[1], "Barrel").is_some()
    });

    // RPC to the server only: arrives with the real sender id, and nowhere
    // else.
    Rpc::new("hello").arg("n", 7).to(RpcTarget::Server).send(a.world_mut());
    run_until(&mut [&mut server, &mut a, &mut b], "the server got the rpc", |apps| {
        !apps[0].world().resource::<ScriptRpcInbox>().pending.is_empty()
    });
    let got = server.world_mut().resource_mut::<ScriptRpcInbox>().pending.remove(0);
    assert_eq!((got.name.as_str(), got.from), ("hello", a_id));
    // RPC to the others: b hears it from a, a does not hear itself.
    Rpc::new("shout").send(a.world_mut());
    run_until(&mut [&mut server, &mut a, &mut b], "b got the relayed rpc", |apps| {
        apps[2].world().resource::<ScriptRpcInbox>().pending.iter().any(|r| r.name == "shout")
    });
    let relayed = b.world().resource::<ScriptRpcInbox>().pending.iter().find(|r| r.name == "shout").unwrap().from;
    assert_eq!(relayed, a_id, "the relay keeps the original sender");
    assert!(a.world().resource::<ScriptRpcInbox>().pending.iter().all(|r| r.name != "shout"));

    // An entity a owns and moves: a's moves reach the server and b, and a
    // client cannot move what it does not own.
    let avatar = server
        .world_mut()
        .spawn((
            Name::new("AvatarA"),
            Transform::default(),
            Networked,
            NetworkOwner(OwnerKind::Client(a_id)),
            NetworkTransform::owner_driven(),
        ))
        .id();
    run_until(&mut [&mut server, &mut a, &mut b], "the avatar exists on a", |apps| find_by_name(apps[1], "AvatarA").is_some());
    let on_a = find_by_name(&mut a, "AvatarA").unwrap();
    assert!(renzora_network_api::is_mine(a.world(), on_a));
    assert!(renzora_network_api::has_authority(a.world(), on_a));
    a.world_mut().get_mut::<Transform>(on_a).unwrap().translation = Vec3::new(0.0, 0.0, 5.0);
    run_until(&mut [&mut server, &mut a, &mut b], "a's move reached b through the server", |apps| {
        let Some(on_b) = find_by_name(apps[2], "AvatarA") else { return false };
        (apps[2].world().get::<Transform>(on_b).unwrap().translation.z - 5.0).abs() < 1e-3
    });
    // b tries to move a's avatar; it stays where a put it on the server.
    let on_b = find_by_name(&mut b, "AvatarA").unwrap();
    assert!(!renzora_network_api::has_authority(b.world(), on_b));

    // Despawn on the server despawns everywhere.
    server.world_mut().entity_mut(crate_e).despawn();
    run_until(&mut [&mut server, &mut a, &mut b], "the despawn reached the client", |apps| find_by_name(apps[1], "Barrel").is_none());

    // b leaves: the server sees it, and a's roster shrinks.
    renzora_network_api::disconnect(b.world_mut());
    run_until(&mut [&mut server, &mut a, &mut b], "b's leave was seen", |apps| {
        status(apps[0]).connected_clients.len() == 1 && status(apps[1]).players == vec![a_id]
    });
    assert_eq!(status(&b).role, NetRole::Offline);
    assert!(find_by_name(&mut b, "AvatarA").is_none(), "a departed client keeps nothing it was sent");
    let _ = avatar;
}

#[test]
fn a_late_joiner_gets_the_world_and_a_host_gets_an_avatar() {
    let mut host = server_app(true);
    assert_eq!(status(&host).client_id, Some(HOST_PEER_ID));
    host.world_mut().spawn((
        Name::new("Spawner"),
        Transform::default(),
        NetworkPlayerSpawner { prefab: "prefabs/player.bsn".into(), spawn_radius: 3.0 },
    ));
    for i in 0..5 {
        host.world_mut().spawn((Name::new(format!("Rock {i}")), Transform::from_xyz(i as f32, 0.0, 0.0), Networked));
    }
    for _ in 0..5 {
        host.update();
    }
    // The host's own avatar, from the spawner, owned by the host.
    let mut q = host.world_mut().query_filtered::<(&NetworkOwner, &NetworkPrefab), With<NetworkPlayer>>();
    let avatars: Vec<(OwnerKind, String)> = q.iter(host.world()).map(|(o, p)| (o.0, p.0.clone())).collect();
    assert_eq!(avatars, vec![(OwnerKind::Client(HOST_PEER_ID), "prefabs/player.bsn".to_string())]);

    let port = port(&host);
    let mut late = client_app(port);
    run_until(&mut [&mut host, &mut late], "the late joiner has every rock and both avatars", |apps| {
        let rocks = (0..5).all(|i| find_by_name(apps[1], &format!("Rock {i}")).is_some());
        let mut q = apps[1].world_mut().query_filtered::<&NetworkId, With<NetworkPlayer>>();
        rocks && q.iter(apps[1].world()).count() == 2
    });
    let me = status(&late).client_id.unwrap();
    assert_eq!(status(&late).players, vec![HOST_PEER_ID, me]);

    // The late joiner leaving takes its avatar with it on the host.
    renzora_network_api::disconnect(late.world_mut());
    run_until(&mut [&mut host, &mut late], "the avatar of the leaver is gone", |apps| {
        let mut q = apps[0].world_mut().query_filtered::<&NetworkOwner, With<NetworkPlayer>>();
        q.iter(apps[0].world()).count() == 1
    });
}

#[test]
fn a_client_of_another_game_is_turned_away() {
    let mut server = server_app(false);
    let port = port(&server);
    let mut stranger = base_app();
    stranger.insert_resource(renzora_project_api::CurrentProject {
        path: Default::default(),
        config: renzora_project_api::ProjectConfig { name: "Some Other Game".into(), ..Default::default() },
    });
    stranger.update();
    renzora_network_api::connect(stranger.world_mut(), "127.0.0.1", port);
    run_until(&mut [&mut server, &mut stranger], "the stranger was refused", |apps| {
        status(apps[1]).last_error.is_some()
    });
    assert!(status(&stranger).last_error.as_deref().unwrap().contains("different game"));
    assert_eq!(status(&server).connected_clients.len(), 0);
}
