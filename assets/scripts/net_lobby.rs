//! Presence tracking through the server-side player lifecycle hooks. The
//! server sees connects and disconnects (`PlayerJoined` / `PlayerLeft` fire
//! only on the server or host), keeps an authoritative count, and broadcasts
//! it so every client's HUD updates too. Nothing is hardcoded.
//!
//! Setup: attach to ONE entity, with `net_connect.rs` and a running server.
//! UI texts named "Players" and "Events" show the count and the last change.

use bevy::prelude::*;
use renzora::network::Rpc;
use renzora::script_bridge::{ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook};

#[derive(Component, Default)]
struct Lobby {
    count: i64,
}

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => ctx.insert(Lobby::default()),
        ScriptHook::PlayerJoined { id } => presence(ctx, *id, true),
        ScriptHook::PlayerLeft { id } => presence(ctx, *id, false),
        // Client side: the server's broadcast.
        ScriptHook::Rpc { name: "lobby", args, .. } => {
            let count = number(args.get("count")).unwrap_or(0);
            let who = number(args.get("who")).map_or_else(|| "?".to_string(), |w| w.to_string());
            let joined = matches!(args.get("joined"), Some(ScriptActionValue::Bool(true)));
            set_text(ctx, "Players", format!("Players: {count}"));
            set_text(ctx, "Events", format!("player {who} {}", if joined { "joined" } else { "left" }));
        }
        _ => {}
    }
}

/// Server side: update the count and tell the clients.
fn presence(ctx: &mut ScriptCtx, id: u64, joined: bool) {
    let Some(mut lobby) = ctx.get_mut::<Lobby>() else { return };
    lobby.count = if joined { lobby.count + 1 } else { (lobby.count - 1).max(0) };
    let count = lobby.count;
    drop(lobby);

    let verb = if joined { "joined" } else { "left" };
    renzora::clog_info!("Script", "[server] player {id} {verb}, {count} online");
    set_text(ctx, "Players", format!("Players: {count}"));
    set_text(ctx, "Events", format!("player {id} {verb}"));
    Rpc::new("lobby").arg("count", count).arg("who", id).arg("joined", joined).send(ctx.world());
}

fn number(value: Option<&ScriptActionValue>) -> Option<i64> {
    match value? {
        ScriptActionValue::Int(i) => Some(*i),
        ScriptActionValue::Float(f) => Some(*f as i64),
        _ => None,
    }
}

fn set_text(ctx: &mut ScriptCtx, name: &str, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", name).arg("text", text).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
