//! A live connection dashboard: this peer's role and connection state, and on
//! the server the player count. The count is only authoritative on the server
//! (or host); clients get it from `net_lobby`'s broadcast instead.
//!
//! Setup: attach to ONE entity. UI texts named "NetStatus" and "Players"
//! show the values; without them the writes are harmless no-ops. Pairs with
//! `net_connect.rs`.

use bevy::prelude::*;
use renzora::network as net;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// What was last written, so the labels are only rewritten on change.
#[derive(Component, Default)]
struct Shown {
    status: String,
    players: Option<usize>,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Shown>() {
        ctx.insert(Shown::default());
    }
    let world = ctx.world();
    let server = net::is_server(world);
    let role = if server { "SERVER" } else { "CLIENT" };
    let conn = if net::is_connected(world) { "connected" } else { "offline" };
    let status = format!("{role} - {conn}");
    let players = server.then(|| net::players(world).len());

    let Some(mut shown) = ctx.get_mut::<Shown>() else { return };
    let status_changed = shown.status != status;
    let players_changed = players.is_some() && shown.players != players;
    shown.status = status.clone();
    shown.players = players;
    drop(shown);

    let me = ctx.entity();
    if status_changed {
        ScriptAction::new("ui_set_text", me).arg("name", "NetStatus").arg("text", status).send(ctx.world());
    }
    if let (true, Some(count)) = (players_changed, players) {
        ScriptAction::new("ui_set_text", me)
            .arg("name", "Players")
            .arg("text", format!("Players: {count}"))
            .send(ctx.world());
    }
}

renzora::script!(update);
