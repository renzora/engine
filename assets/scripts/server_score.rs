//! A server-authoritative scoreboard with a join and leave lifecycle. Trusting
//! clients with the score invites cheating, so only the server mutates the
//! table: clients send "frag" requests, and the server counts them and
//! broadcasts the canonical board back. The join and leave hooks keep the
//! roster honest across reconnects.
//!
//! RPC arguments hold no lists, so the board travels as one JSON string,
//! `{"<peer id>": points, ...}`. The Lua original referred to a
//! `request_point()` it never defined; here a client presses P to send one.
//!
//! Setup: attach the same script on the host and on every client. A UI text
//! named "score_leader" shows the leader on clients.

use std::collections::BTreeMap;

use bevy::prelude::*;
use renzora::network::{self as net, Rpc, RpcArgs, RpcTarget};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

#[derive(Component, Default)]
struct Scoreboard {
    /// Peer id to points. Only ever written on the server.
    scores: BTreeMap<u64, i64>,
    dirty: bool,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Scoreboard>() {
        ctx.insert(Scoreboard::default());
    }
    let request = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::KeyP));
    if request && net::is_client(ctx.world()) {
        request_point(ctx.world());
    }
    if !net::is_server(ctx.world()) {
        return;
    }
    let Some(mut board) = ctx.get_mut::<Scoreboard>() else { return };
    if !board.dirty {
        return;
    }
    board.dirty = false;
    let snapshot: BTreeMap<String, i64> = board.scores.iter().map(|(id, p)| (id.to_string(), *p)).collect();
    drop(board);
    let Ok(json) = renzora::serde_json::to_string(&snapshot) else { return };
    Rpc::new("board").arg("scores", json).send(ctx.world());
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::PlayerJoined { id } => {
            if net::is_server(ctx.world()) {
                edit(ctx, |board| {
                    board.scores.entry(*id).or_insert(0);
                });
            }
        }
        ScriptHook::PlayerLeft { id } => {
            if net::is_server(ctx.world()) {
                edit(ctx, |board| {
                    board.scores.remove(id);
                });
            }
        }
        // The server is the only place a point is ever minted.
        ScriptHook::Rpc { name: "frag", from, .. } => {
            if net::is_server(ctx.world()) {
                edit(ctx, |board| *board.scores.entry(*from).or_insert(0) += 1);
            }
        }
        ScriptHook::Rpc { name: "board", args, .. } => {
            if !net::is_client(ctx.world()) {
                return;
            }
            let Some(json) = args.str("scores") else { return };
            let Ok(scores) = renzora::serde_json::from_str::<BTreeMap<String, i64>>(json) else { return };
            let Some((id, points)) = scores.iter().max_by_key(|(_, p)| **p) else { return };
            let me = ctx.entity();
            ScriptAction::new("ui_set_text", me)
                .arg("name", "score_leader")
                .arg("text", format!("Leader peer#{id} : {points}"))
                .send(ctx.world());
        }
        _ => {}
    }
}

/// Change the board and mark it for the next broadcast.
fn edit(ctx: &mut ScriptCtx, change: impl FnOnce(&mut Scoreboard)) {
    if !ctx.has::<Scoreboard>() {
        ctx.insert(Scoreboard::default());
    }
    if let Some(mut board) = ctx.get_mut::<Scoreboard>() {
        change(&mut board);
        board.dirty = true;
    }
}

/// Ask the server for a point; it decides whether it counts.
fn request_point(world: &mut World) {
    Rpc::new("frag").to(RpcTarget::Server).send(world);
}

renzora::script!(update, hooks = hooks);
