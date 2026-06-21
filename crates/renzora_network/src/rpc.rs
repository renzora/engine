//! Networked RPCs — the `rpc("name", { args })` path.
//!
//! Scripts call `rpc(name, args)`, which the scripting layer turns into a
//! `ScriptAction` named `net_rpc`. [`handle_network_script_actions`](crate::script_extension::handle_network_script_actions)
//! enqueues it into [`PendingOutgoingRpc`]; [`send_outgoing_rpcs`] serializes
//! the args into a [`GameEvent`] and puts it on the wire (to the server if
//! we're a client, or to every client if we're the server).
//!
//! On receipt, the client/server poll systems decode the `GameEvent` into a
//! [`renzora::IncomingRpc`] and drop it in [`renzora::ScriptRpcInbox`], where
//! `renzora_scripting` drains it and fires each script's `on_rpc(name, args)`
//! hook. When we're the server we *also* relay the event to every other
//! connected client, so a client→server→clients fan-out works.
//!
//! v1 semantics: broadcast to all scripts, no local echo, no authority checks,
//! reliable channel. Targeting/authority are layered on later.

use std::collections::HashMap;

use bevy::prelude::*;

use renzora::ScriptActionValue;

use crate::messages::GameEvent;

/// Reserved arg key carrying the RPC name. The `rpc()` script verb writes it;
/// the network layer strips it before the args become the payload.
pub const RPC_NAME_KEY: &str = "__rpc";

/// Queue of RPCs produced by scripts this frame, awaiting send.
#[derive(Resource, Default)]
pub struct PendingOutgoingRpc {
    pub queue: Vec<OutgoingRpc>,
}

/// One RPC to put on the wire.
pub struct OutgoingRpc {
    pub name: String,
    pub args: HashMap<String, ScriptActionValue>,
}

// ── Serialization ───────────────────────────────────────────────────────────
//
// The arg bag rides in `GameEvent.data` as JSON. JSON can't tell an integer
// from a float, so decode picks `Int` for whole numbers and `Float` otherwise
// — Lua sees a number either way. A 3-element number array round-trips as
// `Vec3`, matching how `lua_to_action_value` packs vectors.

/// Encode an arg bag to JSON bytes for `GameEvent.data`.
pub fn args_to_bytes(args: &HashMap<String, ScriptActionValue>) -> Vec<u8> {
    use serde_json::{Map, Number, Value};
    let mut map = Map::with_capacity(args.len());
    for (k, v) in args {
        let jv = match v {
            ScriptActionValue::Float(f) => Number::from_f64(*f as f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            ScriptActionValue::Int(i) => Value::Number(Number::from(*i)),
            ScriptActionValue::Bool(b) => Value::Bool(*b),
            ScriptActionValue::String(s) => Value::String(s.clone()),
            ScriptActionValue::Vec3(xyz) => Value::Array(
                xyz.iter()
                    .filter_map(|c| Number::from_f64(*c as f64).map(Value::Number))
                    .collect(),
            ),
        };
        map.insert(k.clone(), jv);
    }
    serde_json::to_vec(&Value::Object(map)).unwrap_or_default()
}

/// Decode `GameEvent.data` JSON bytes back into an arg bag.
pub fn args_from_bytes(data: &[u8]) -> HashMap<String, ScriptActionValue> {
    use serde_json::Value;
    let mut out = HashMap::new();
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(data) else {
        return out;
    };
    for (k, v) in map {
        let val = match v {
            Value::Bool(b) => ScriptActionValue::Bool(b),
            Value::String(s) => ScriptActionValue::String(s),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ScriptActionValue::Int(i)
                } else {
                    ScriptActionValue::Float(n.as_f64().unwrap_or(0.0) as f32)
                }
            }
            Value::Array(arr) if arr.len() == 3 && arr.iter().all(|e| e.is_number()) => {
                let mut xyz = [0.0f32; 3];
                for (i, e) in arr.iter().enumerate() {
                    xyz[i] = e.as_f64().unwrap_or(0.0) as f32;
                }
                ScriptActionValue::Vec3(xyz)
            }
            _ => continue,
        };
        out.insert(k, val);
    }
    out
}

// ── Send ────────────────────────────────────────────────────────────────────

/// Drain [`PendingOutgoingRpc`] and put each RPC on the wire. A client sends to
/// the server; a server broadcasts to every connected client. (The server also
/// relays *received* client RPCs to the other clients in `server_poll`.)
pub fn send_outgoing_rpcs(
    mut pending: ResMut<PendingOutgoingRpc>,
    mut client: Option<ResMut<crate::client::NetworkClient>>,
    mut server: Option<ResMut<crate::server::NetworkServer>>,
) {
    if pending.queue.is_empty() {
        return;
    }
    for rpc in pending.queue.drain(..) {
        let event = GameEvent {
            name: rpc.name,
            data: args_to_bytes(&rpc.args),
        };
        if let Some(client) = client.as_mut() {
            client.send_event(event);
        } else if let Some(server) = server.as_mut() {
            server.broadcast(event, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::ScriptActionValue;

    #[test]
    fn args_round_trip_preserves_types() {
        let mut args = HashMap::new();
        args.insert("count".into(), ScriptActionValue::Int(7));
        args.insert("speed".into(), ScriptActionValue::Float(1.5));
        args.insert("alive".into(), ScriptActionValue::Bool(true));
        args.insert("name".into(), ScriptActionValue::String("bob".into()));
        args.insert("pos".into(), ScriptActionValue::Vec3([1.0, 2.0, 3.0]));

        let bytes = args_to_bytes(&args);
        let back = args_from_bytes(&bytes);

        assert_eq!(back.len(), 5);
        assert!(matches!(back["count"], ScriptActionValue::Int(7)));
        assert!(matches!(back["alive"], ScriptActionValue::Bool(true)));
        match &back["speed"] {
            ScriptActionValue::Float(f) => assert!((f - 1.5).abs() < 1e-6),
            other => panic!("speed decoded as {other:?}"),
        }
        match &back["name"] {
            ScriptActionValue::String(s) => assert_eq!(s, "bob"),
            other => panic!("name decoded as {other:?}"),
        }
        match &back["pos"] {
            ScriptActionValue::Vec3(v) => assert_eq!(*v, [1.0, 2.0, 3.0]),
            other => panic!("pos decoded as {other:?}"),
        }
    }

    #[test]
    fn empty_args_round_trip() {
        let args = HashMap::new();
        assert!(args_from_bytes(&args_to_bytes(&args)).is_empty());
    }

    #[test]
    fn garbage_bytes_decode_to_empty() {
        assert!(args_from_bytes(b"not json at all").is_empty());
        assert!(args_from_bytes(&[]).is_empty());
    }
}
