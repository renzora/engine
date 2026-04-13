//! Network script integration — handles networking actions from scripts.
//!
//! Scripts use the generic `action()` function for network operations:
//!
//! ```lua
//! action("net_send", { channel = "chat", data = "hello" })
//! action("net_spawn", { name = "Bullet", x = 0, y = 1, z = 0 })
//! action("net_rpc", { name = "damage", args = "25" })
//! ```

use bevy::prelude::*;
use renzora::ScriptAction;

use crate::status::NetworkStatus;

/// Observer that handles network-related ScriptAction events.
pub fn handle_network_script_actions(
    trigger: On<ScriptAction>,
    status: Res<NetworkStatus>,
    mut cmds: Commands,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();

    let get_str = |key: &str| -> String {
        match action.args.get(key) {
            Some(ScriptActionValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    };
    let get_f32 = |key: &str| -> f32 {
        match action.args.get(key) {
            Some(ScriptActionValue::Float(v)) => *v,
            Some(ScriptActionValue::Int(v)) => *v as f32,
            _ => 0.0,
        }
    };
    let get_i64 = |key: &str| -> i64 {
        match action.args.get(key) {
            Some(ScriptActionValue::Int(v)) => *v,
            Some(ScriptActionValue::Float(v)) => *v as i64,
            _ => 0,
        }
    };

    match action.name.as_str() {
        "net_connect" => {
            let address = get_str("address");
            let port = get_i64("port") as u16;
            if address.is_empty() || port == 0 {
                log::warn!("[network] net_connect: invalid address/port");
                return;
            }
            log::info!("[network] Connect request: {}:{}", address, port);
            cmds.insert_resource(crate::PendingNetworkConnect { address, port });
        }
        "net_disconnect" => {
            log::info!("[network] Disconnect request");
            cmds.insert_resource(crate::PendingNetworkDisconnect);
        }
        "net_host_server" => {
            let port = get_i64("port") as u16;
            let max_clients = get_i64("max_clients") as u16;
            log::info!(
                "[network] Host server request (port={}, max={}). \
                 Use renzora-server binary for dedicated servers.",
                port, max_clients
            );
        }
        "net_send" | "net_send_message" => {
            if !status.is_connected() && !status.is_server {
                log::warn!("[network] Script send ignored — not connected");
                return;
            }
            let name = get_str("channel");
            let data = get_str("data");
            log::info!("[network] Script send event: {} ({}B)", name, data.len());
            // TODO: send via Lightyear connection
        }
        "net_spawn" => {
            if !status.is_connected() && !status.is_server {
                log::warn!("[network] Script spawn ignored — not connected");
                return;
            }
            let name = get_str("name");
            let x = get_f32("x");
            let y = get_f32("y");
            let z = get_f32("z");
            log::info!("[network] Script spawn request: {} at ({}, {}, {})", name, x, y, z);
            // TODO: send SpawnRequest to server
        }
        "net_rpc" => {
            if !status.is_connected() && !status.is_server {
                log::warn!("[network] Script RPC ignored — not connected");
                return;
            }
            let name = get_str("name");
            let args = get_str("args");
            log::info!("[network] Script RPC: {} ({}B)", name, args.len());
            // TODO: send as GameEvent message
        }
        _ => {} // Not a network action
    }
}
