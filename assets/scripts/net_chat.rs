//! Networked chat with preset lines. Keys 1, 2 and 3 broadcast a line to every
//! other peer, and each receiver logs it with the sender's peer id and shows
//! it on the HUD. An RPC never echoes to its sender, so the sender echoes its
//! own line locally.
//!
//! Setup: attach to any entity, with `net_connect.rs` on one entity and a
//! running server. A UI text named "Chat" shows the newest line (optional;
//! the Console log works without it).

use bevy::prelude::*;
use renzora::network::Rpc;
use renzora::script_bridge::{ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// The line each digit key sends.
const LINES: [(KeyCode, &str); 3] = [
    (KeyCode::Digit1, "hello!"),
    (KeyCode::Digit2, "gg"),
    (KeyCode::Digit3, "on my way"),
];

fn update(ctx: &mut ScriptCtx) {
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let said: Vec<&str> = LINES
        .iter()
        .filter(|(key, _)| input.is_key_just_pressed(*key))
        .map(|(_, line)| *line)
        .collect();
    for text in said {
        Rpc::new("chat").arg("text", text).send(ctx.world());
        show(ctx, format!("you: {text}"));
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Rpc { name: "chat", args, from } = hook {
        let text = match args.get("text") {
            Some(ScriptActionValue::String(s)) => s.clone(),
            _ => "nil".to_string(),
        };
        show(ctx, format!("[player {from}] {text}"));
    }
}

fn show(ctx: &mut ScriptCtx, line: String) {
    renzora::clog_info!("Script", "{line}");
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", "Chat").arg("text", line).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
