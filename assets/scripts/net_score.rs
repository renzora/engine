//! A server-authoritative score. Any peer presses the key to request a point;
//! only the server owns the count, increments it and broadcasts the new total.
//! Clients ask, the server decides.
//!
//! Setup: attach to any entity, with `net_connect.rs` and a running server. A
//! UI text named "Score" shows the total (optional).

use bevy::prelude::*;
use renzora::network::{self as net, Rpc};
use renzora::script_bridge::{ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const ADD_KEY: KeyCode = KeyCode::KeyK;

/// The server-side total.
#[derive(Component, Default)]
struct Score(i64);

fn update(ctx: &mut ScriptCtx) {
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(ADD_KEY));
    if pressed {
        // A broadcast; only the server acts on it.
        Rpc::new("score_request").send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => ctx.insert(Score::default()),
        ScriptHook::Rpc { name: "score_request", from, .. } => {
            if !net::is_server(ctx.world()) {
                return;
            }
            let Some(mut score) = ctx.get_mut::<Score>() else { return };
            score.0 += 1;
            let total = score.0;
            drop(score);
            Rpc::new("score_update").arg("total", total).send(ctx.world());
            show(ctx, total);
            renzora::clog_info!("Script", "player {from} scored, total {total}");
        }
        ScriptHook::Rpc { name: "score_update", args, .. } => {
            let total = match args.get("total") {
                Some(ScriptActionValue::Int(i)) => *i,
                Some(ScriptActionValue::Float(f)) => *f as i64,
                _ => return,
            };
            show(ctx, total);
            renzora::clog_info!("Script", "SCORE: {total}");
        }
        _ => {}
    }
}

fn show(ctx: &mut ScriptCtx, total: i64) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me)
        .arg("name", "Score")
        .arg("text", format!("Score: {total}"))
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
