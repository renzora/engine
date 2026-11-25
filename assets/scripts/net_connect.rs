//! Connects this client to the dedicated server on play, and nothing else.
//! Connection is kept separate from gameplay so the other networking scripts
//! (`multiplayer_ping`, `net_chat`, ...) can assume it already exists.
//!
//! Setup: attach to exactly ONE entity (e.g. an empty named "net"); two
//! connectors would try to open two connections. Run the server first with
//! `renzora-runtime --server`.

use bevy::prelude::*;
use renzora::network as net;
use renzora::{ScriptCtx, ScriptHook};

const ADDRESS: &str = "127.0.0.1";
/// Matches the server's `--port` / project.toml.
const PORT: u16 = 7636;

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if matches!(hook, ScriptHook::Ready) {
        net::connect(ctx.world(), ADDRESS, PORT);
        renzora::clog_info!("Script", "connecting to {ADDRESS}:{PORT}");
    }
}

renzora::script!(update, hooks = hooks);
