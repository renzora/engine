//! A multiplayer text chat built on RPCs. A "chat" is a broadcast of the
//! sender's name and message, which every peer mirrors into a rolling log.
//!
//! Setup: attach to an always-loaded manager entity and connect first. The
//! newest line is shown on a UI text named "chat_line", if one exists.

use bevy::prelude::*;
use renzora::network::{self as net, Rpc};
use renzora::script_bridge::{ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook};

/// Lines kept in the backlog.
const HISTORY: usize = 8;

#[derive(Component, Default)]
struct Chat {
    log: Vec<String>,
    greeted: bool,
}

fn update(ctx: &mut ScriptCtx) {
    if !net::is_connected(ctx.world()) {
        return;
    }
    let greeted = ctx.get::<Chat>().is_some_and(|c| c.greeted);
    if greeted {
        return;
    }
    if let Some(mut chat) = ctx.get_mut::<Chat>() {
        chat.greeted = true;
    }
    let me = ctx.name().unwrap_or("someone").to_string();
    Rpc::new("chat").arg("name", me).arg("msg", "joined the channel").send(ctx.world());
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Chat::default());
            let line = format!("[chat] ready as {}", ctx.name().unwrap_or("?"));
            push(ctx, line);
        }
        ScriptHook::Rpc { name: "chat", args, from } => {
            let who = match args.get("name") {
                Some(ScriptActionValue::String(s)) => s.clone(),
                _ => format!("peer#{from}"),
            };
            let msg = match args.get("msg") {
                Some(ScriptActionValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            push(ctx, format!("{who}: {msg}"));
        }
        ScriptHook::PlayerLeft { id } => push(ctx, format!("[chat] peer#{id} left")),
        _ => {}
    }
}

/// Add a line to the backlog and show it.
fn push(ctx: &mut ScriptCtx, line: String) {
    if let Some(mut chat) = ctx.get_mut::<Chat>() {
        chat.log.push(line.clone());
        let excess = chat.log.len().saturating_sub(HISTORY);
        chat.log.drain(..excess);
    }
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", "chat_line").arg("text", line).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
