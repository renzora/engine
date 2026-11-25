//! A networked ping: the smallest RPC round trip. Pressing the ping key
//! broadcasts an RPC to every other connected peer; it never echoes back to
//! the sender, so the sender logs "sent ping" and each remote peer logs
//! "got ping".
//!
//! Setup: attach to any entity, and attach `net_connect.rs` to one entity as
//! well. Run the server first (`renzora-runtime --server`), then watch the
//! consoles.

use bevy::prelude::*;
use renzora::network::Rpc;
use renzora::script_bridge::ScriptActionValue;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const PING_KEY: KeyCode = KeyCode::KeyP;
/// Text sent with each ping.
const MESSAGE: &str = "hello";

fn update(ctx: &mut ScriptCtx) {
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(PING_KEY));
    if !pressed {
        return;
    }
    let at = ctx.elapsed();
    // Not connected is a no-op with a warning, so no gate is needed here.
    Rpc::new("ping").arg("msg", MESSAGE).arg("at", at).send(ctx.world());
    renzora::clog_info!("Script", "sent ping: {MESSAGE}");
}

fn hooks(_ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Rpc { name: "ping", args, from } = hook {
        let msg = match args.get("msg") {
            Some(ScriptActionValue::String(s)) => s.as_str(),
            _ => "nil",
        };
        renzora::clog_info!("Script", "got ping from {from}: {msg}");
    }
}

renzora::script!(update, hooks = hooks);
