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
use renzora_scripting_api::ScriptAction;

use crate::status::NetworkStatus;

/// Observer that handles network-related ScriptAction events.
pub fn handle_network_script_actions(
    trigger: On<ScriptAction>,
    status: Res<NetworkStatus>,
    project: Option<Res<renzora_project_api::CurrentProject>>,
    mut cmds: Commands,
    mut outbox: ResMut<crate::rpc::PendingOutgoingRpc>,
) {
    use renzora_scripting_api::ScriptActionValue;
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
            // Whatever the call leaves out comes from the project's `[network]`
            // section, so a game connects to the server it was configured for
            // with a bare `net_connect`, and a script names an address only to
            // go somewhere else.
            let (address, port) = connect_target(
                get_str("address"),
                get_i64("port"),
                project.as_ref().and_then(|p| p.config.network.as_ref()),
            );
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
                 Run the runtime with `--server` for a dedicated server.",
                port,
                max_clients
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
            log::info!(
                "[network] Script spawn request: {} at ({}, {}, {})",
                name,
                x,
                y,
                z
            );
            // TODO: send SpawnRequest to server
        }
        "net_rpc" => {
            if !status.is_connected() && !status.is_server {
                log::warn!("[network] Script RPC ignored — not connected");
                return;
            }
            // Name comes from the reserved key set by the `rpc()` verb, or
            // falls back to the legacy `action("net_rpc", { name = .. })` form.
            let name_key = if action.args.contains_key(crate::rpc::RPC_NAME_KEY) {
                crate::rpc::RPC_NAME_KEY
            } else {
                "name"
            };
            let name = match action.args.get(name_key) {
                Some(ScriptActionValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            if name.is_empty() {
                log::warn!("[network] Script RPC ignored — missing name");
                return;
            }
            // Everything except the reserved name key is the payload.
            let payload: std::collections::HashMap<String, ScriptActionValue> = action
                .args
                .iter()
                .filter(|(k, _)| k.as_str() != name_key)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            log::debug!("[network] Script RPC: {} ({} args)", name, payload.len());
            outbox.queue.push(crate::rpc::OutgoingRpc {
                name,
                args: payload,
            });
        }
        _ => {} // Not a network action
    }
}

/// Where a `net_connect` goes: the address and port the script gave, with
/// either one it left out (empty, or a port outside 1..=65535) taken from the
/// project's `[network]` section, and that section's defaults when the project
/// has none.
fn connect_target(
    address: String,
    port: i64,
    project: Option<&renzora_project_api::NetworkProjectConfig>,
) -> (String, u16) {
    let fallback = project.cloned().unwrap_or_default();
    let address = if address.trim().is_empty() { fallback.server_addr } else { address };
    let port = u16::try_from(port).ok().filter(|p| *p != 0).unwrap_or(fallback.port);
    (address, port)
}

#[cfg(test)]
mod tests {
    use super::connect_target;
    use renzora_project_api::NetworkProjectConfig;

    fn project() -> NetworkProjectConfig {
        NetworkProjectConfig { server_addr: "play.example.net".into(), port: 9000, ..Default::default() }
    }

    #[test]
    fn a_bare_connect_goes_to_the_projects_server() {
        assert_eq!(connect_target(String::new(), 0, Some(&project())), ("play.example.net".into(), 9000));
    }

    #[test]
    fn what_the_script_names_wins() {
        assert_eq!(connect_target("10.0.0.5".into(), 7000, Some(&project())), ("10.0.0.5".into(), 7000));
        assert_eq!(connect_target(String::new(), 7000, Some(&project())), ("play.example.net".into(), 7000));
    }

    #[test]
    fn no_network_section_uses_its_defaults() {
        let d = NetworkProjectConfig::default();
        assert_eq!(connect_target(String::new(), -1, None), (d.server_addr, d.port));
    }
}
