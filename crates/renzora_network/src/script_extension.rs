//! Networking from a script language backend.
//!
//! A Rust script calls `renzora::network` directly. A language plugin (Lua,
//! from the marketplace) cannot, so its `rpc()` and `action("net_*", ...)`
//! verbs arrive here as `ScriptAction`s and are put in the same
//! [`NetOutbox`] a Rust script writes.
//!
//! ```lua
//! rpc("pickup", { coin = id, __to = "server" })   -- __to: server | all | others | <peer id>
//! action("net_connect", { address = "10.0.0.5", port = 7636 })
//! action("net_disconnect")
//! action("net_spawn", { prefab = "prefabs/coin.bsn", x = 0, y = 1, z = 0 })  -- server only
//! ```

use std::collections::HashMap;

use bevy::prelude::*;
use renzora_network_api::{Delivery, NetOutbox, NetRequest, OutgoingRpc, OwnerKind, RpcTarget};
use renzora_scripting_api::{ScriptAction, ScriptActionValue};

/// The argument the `rpc()` verb carries the RPC's name in.
pub const RPC_NAME_KEY: &str = "__rpc";
/// Who the RPC is for: `"server"`, `"all"`, `"others"`, or a peer id.
pub const RPC_TARGET_KEY: &str = "__to";
/// `true` to send unreliably.
pub const RPC_UNRELIABLE_KEY: &str = "__unreliable";

pub fn handle_network_script_actions(trigger: On<ScriptAction>, mut commands: Commands) {
    let action = trigger.event();
    let str_arg = |key: &str| match action.args.get(key) {
        Some(ScriptActionValue::String(s)) => s.clone(),
        _ => String::new(),
    };
    let num_arg = |key: &str| match action.args.get(key) {
        Some(ScriptActionValue::Float(v)) => *v as f64,
        Some(ScriptActionValue::Int(v)) => *v as f64,
        _ => 0.0,
    };

    match action.name.as_str() {
        "net_connect" => {
            let request = NetRequest::Connect {
                address: str_arg("address"),
                port: u16::try_from(num_arg("port") as i64).unwrap_or(0),
            };
            commands.queue(move |world: &mut World| {
                world.get_resource_or_insert_with(NetOutbox::default).requests.push(request);
            });
        }
        "net_disconnect" => {
            commands.queue(|world: &mut World| {
                world.get_resource_or_insert_with(NetOutbox::default).requests.push(NetRequest::Disconnect);
            });
        }
        "net_rpc" => {
            let Some(rpc) = parse_rpc(&action.args) else {
                warn!("[network] rpc() ignored: it has no name");
                return;
            };
            commands.queue(move |world: &mut World| {
                world.get_resource_or_insert_with(NetOutbox::default).rpcs.push(rpc);
            });
        }
        "net_spawn" => {
            let prefab = match str_arg("prefab") {
                p if !p.is_empty() => p,
                _ => str_arg("name"),
            };
            let at = Vec3::new(num_arg("x") as f32, num_arg("y") as f32, num_arg("z") as f32);
            commands.queue(move |world: &mut World| {
                renzora_network_api::spawn_prefab(world, prefab, Transform::from_translation(at), OwnerKind::Server);
            });
        }
        "net_send" | "net_send_message" | "net_host_server" => {
            warn!(
                "[network] `{}` is not a network verb any more: send with rpc(), host with `--host`",
                action.name
            );
        }
        _ => {}
    }
}

/// An `rpc()` call's arguments to an outgoing RPC. The legacy
/// `action("net_rpc", { name = ... })` form is still read.
pub fn parse_rpc(args: &HashMap<String, ScriptActionValue>) -> Option<OutgoingRpc> {
    let name_key = if args.contains_key(RPC_NAME_KEY) { RPC_NAME_KEY } else { "name" };
    let name = match args.get(name_key) {
        Some(ScriptActionValue::String(s)) if !s.is_empty() => s.clone(),
        _ => return None,
    };
    let target = match args.get(RPC_TARGET_KEY) {
        Some(ScriptActionValue::String(s)) => match s.to_ascii_lowercase().as_str() {
            "server" => RpcTarget::Server,
            "all" => RpcTarget::All,
            _ => RpcTarget::Others,
        },
        Some(ScriptActionValue::Int(id)) => RpcTarget::Peer(*id as u64),
        _ => RpcTarget::Others,
    };
    let delivery = match args.get(RPC_UNRELIABLE_KEY) {
        Some(ScriptActionValue::Bool(true)) => Delivery::Unreliable,
        _ => Delivery::Reliable,
    };
    let payload = args
        .iter()
        .filter(|(k, _)| ![name_key, RPC_TARGET_KEY, RPC_UNRELIABLE_KEY].contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Some(OutgoingRpc { target, delivery, name, args: payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, ScriptActionValue)]) -> HashMap<String, ScriptActionValue> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn the_reserved_keys_are_read_and_stripped() {
        let rpc = parse_rpc(&args(&[
            (RPC_NAME_KEY, ScriptActionValue::String("hit".into())),
            (RPC_TARGET_KEY, ScriptActionValue::String("Server".into())),
            (RPC_UNRELIABLE_KEY, ScriptActionValue::Bool(true)),
            ("damage", ScriptActionValue::Int(3)),
        ]))
        .unwrap();
        assert_eq!(rpc.name, "hit");
        assert_eq!(rpc.target, RpcTarget::Server);
        assert_eq!(rpc.delivery, Delivery::Unreliable);
        assert_eq!(rpc.args.len(), 1);
    }

    #[test]
    fn the_old_form_still_works_and_defaults_to_others() {
        let rpc = parse_rpc(&args(&[("name", ScriptActionValue::String("ping".into()))])).unwrap();
        assert_eq!(rpc.target, RpcTarget::Others);
        assert!(rpc.args.is_empty());
        assert!(parse_rpc(&HashMap::new()).is_none());
    }

    #[test]
    fn a_number_targets_one_peer() {
        let rpc = parse_rpc(&args(&[
            (RPC_NAME_KEY, ScriptActionValue::String("dm".into())),
            (RPC_TARGET_KEY, ScriptActionValue::Int(99)),
        ]))
        .unwrap();
        assert_eq!(rpc.target, RpcTarget::Peer(99));
    }
}
