//! What the game says over the transport: the messages inside a reliable or
//! unreliable payload, and how they are encoded.
//!
//! Reliable messages carry everything that must arrive and arrive in order:
//! spawns, despawns, component changes, the player roster, RPCs. Unreliable
//! ones carry transforms, which are sent again a moment later anyway, so a
//! lost one is replaced rather than resent.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora_network_api::RpcTarget;
use renzora_scripting_api::ScriptActionValue;
use serde::{Deserialize, Serialize};

/// A component on the wire: its type path and its reflected value as JSON.
///
/// JSON because the reflection deserializer relies on a self-describing
/// format for maps and enums, and because a component that fails to decode
/// should fail with a readable error. Components change rarely; transforms,
/// which change every tick, go in [`WireTransform`] instead.
pub type WireComponent = (String, Vec<u8>);

/// A transform, with only the parts its entity replicates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WireTransform {
    pub translation: [f32; 3],
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

impl WireTransform {
    pub fn from_transform(t: &Transform, rotation: bool, scale: bool) -> Self {
        Self {
            translation: t.translation.to_array(),
            rotation: rotation.then(|| t.rotation.to_array()),
            scale: scale.then(|| t.scale.to_array()),
        }
    }

    /// Write into `t`, leaving whatever this does not carry alone.
    pub fn apply(&self, t: &mut Transform) {
        t.translation = Vec3::from_array(self.translation);
        if let Some(r) = self.rotation {
            t.rotation = Quat::from_array(r).normalize();
        }
        if let Some(s) = self.scale {
            t.scale = Vec3::from_array(s);
        }
    }

    /// Whether `other` differs enough to be worth sending. A threshold rather
    /// than equality, so floating-point noise from a physics step at rest
    /// does not stream a still object forever.
    pub fn differs(&self, other: &Self) -> bool {
        let d = |a: &[f32], b: &[f32]| a.iter().zip(b).any(|(x, y)| (x - y).abs() > 1e-4);
        d(&self.translation, &other.translation)
            || match (self.rotation, other.rotation) {
                (Some(a), Some(b)) => d(&a, &b),
                (a, b) => a.is_some() != b.is_some(),
            }
            || match (self.scale, other.scale) {
                (Some(a), Some(b)) => d(&a, &b),
                (a, b) => a.is_some() != b.is_some(),
            }
    }

    /// Blend between two samples. Translation and scale linearly, rotation by
    /// the shortest arc.
    pub fn lerp(&self, other: &Self, s: f32) -> Self {
        let lerp3 = |a: [f32; 3], b: [f32; 3]| Vec3::from_array(a).lerp(Vec3::from_array(b), s).to_array();
        Self {
            translation: lerp3(self.translation, other.translation),
            rotation: match (self.rotation, other.rotation) {
                (Some(a), Some(b)) => Some(Quat::from_array(a).slerp(Quat::from_array(b), s).to_array()),
                (_, b) => b,
            },
            scale: match (self.scale, other.scale) {
                (Some(a), Some(b)) => Some(lerp3(a, b)),
                (_, b) => b,
            },
        }
    }
}

/// [`RpcTarget`], in a shape serde can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireTarget {
    Server,
    All,
    Others,
    Peer(u64),
}

impl From<RpcTarget> for WireTarget {
    fn from(t: RpcTarget) -> Self {
        match t {
            RpcTarget::Server => Self::Server,
            RpcTarget::All => Self::All,
            RpcTarget::Others => Self::Others,
            RpcTarget::Peer(p) => Self::Peer(p),
        }
    }
}

/// A replicated entity as a newcomer first sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnData {
    pub net_id: u64,
    /// Set when the entity came from the scene file, so a client that loaded
    /// the same scene adopts its own copy instead of spawning a second.
    pub scene_id: Option<u64>,
    pub transform: WireTransform,
    pub components: Vec<WireComponent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NetMessage {
    /// Server → client: everyone in the session, the receiver included.
    Roster { players: Vec<u64> },
    /// Server → client.
    Spawn(SpawnData),
    /// Server → client.
    Despawn { net_id: u64 },
    /// Server → client: components that changed or went away.
    Update { net_id: u64, changed: Vec<WireComponent>, removed: Vec<String> },
    /// Server → client: where things are at server time `time` (seconds).
    Snapshot { time: f64, transforms: Vec<(u64, WireTransform)> },
    /// Client → server: where the client's own owner-moved entities are, at
    /// the client's estimate of server time.
    Owned { time: f64, transforms: Vec<(u64, WireTransform)> },
    /// Either way. `from` is stamped by the server on relay: a client's own
    /// claim is never trusted.
    Rpc { from: u64, target: WireTarget, name: String, args: Vec<u8> },
}

pub fn encode(msg: &NetMessage) -> Vec<u8> {
    bincode::serde::encode_to_vec(msg, bincode::config::standard()).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<NetMessage> {
    // Bounded: a reassembled reliable message can be large, but not this
    // large, and a forged length must not allocate without limit.
    let config = bincode::config::standard().with_limit::<{ 16 * 1024 * 1024 }>();
    bincode::serde::decode_from_slice(bytes, config).ok().map(|(m, _)| m)
}

/// The most transforms in one snapshot message, keeping it under the
/// unreliable size limit (a transform with rotation and scale is about 50
/// bytes encoded).
pub const TRANSFORMS_PER_MESSAGE: usize = 18;

// ── RPC arguments ───────────────────────────────────────────────────────────
//
// The argument bag rides as JSON, so a Rust sender and a Lua receiver agree.
// JSON cannot tell an integer from a float, so decode picks `Int` for whole
// numbers and `Float` otherwise; `RpcArgs` reads either as either. A three-
// number array is a `Vec3`, matching how scripts pack vectors.

pub fn args_to_bytes(args: &HashMap<String, ScriptActionValue>) -> Vec<u8> {
    use serde_json::{Map, Number, Value};
    let mut map = Map::with_capacity(args.len());
    for (k, v) in args {
        let jv = match v {
            ScriptActionValue::Float(f) => Number::from_f64(*f as f64).map(Value::Number).unwrap_or(Value::Null),
            ScriptActionValue::Int(i) => Value::Number(Number::from(*i)),
            ScriptActionValue::Bool(b) => Value::Bool(*b),
            ScriptActionValue::String(s) => Value::String(s.clone()),
            ScriptActionValue::Vec3(xyz) => Value::Array(
                xyz.iter().filter_map(|c| Number::from_f64(*c as f64).map(Value::Number)).collect(),
            ),
        };
        map.insert(k.clone(), jv);
    }
    serde_json::to_vec(&Value::Object(map)).unwrap_or_default()
}

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
            Value::Number(n) => match n.as_i64() {
                Some(i) => ScriptActionValue::Int(i),
                None => ScriptActionValue::Float(n.as_f64().unwrap_or(0.0) as f32),
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let m = NetMessage::Spawn(SpawnData {
            net_id: 9,
            scene_id: Some(77),
            transform: WireTransform { translation: [1.0, 2.0, 3.0], rotation: Some([0.0, 0.0, 0.0, 1.0]), scale: None },
            components: vec![("bevy_ecs::name::Name".into(), b"\"Bob\"".to_vec())],
        });
        assert_eq!(decode(&encode(&m)), Some(m));
    }

    #[test]
    fn a_full_snapshot_message_fits_unreliably() {
        let t = WireTransform { translation: [1e6; 3], rotation: Some([0.7; 4]), scale: Some([2.0; 3]) };
        let m = NetMessage::Snapshot { time: 1e9, transforms: vec![(u64::MAX, t); TRANSFORMS_PER_MESSAGE] };
        assert!(encode(&m).len() <= crate::transport::packet::MAX_UNRELIABLE_BYTES);
    }

    #[test]
    fn transforms_ignore_noise_but_not_motion() {
        let a = WireTransform::from_transform(&Transform::from_xyz(1.0, 0.0, 0.0), true, false);
        let mut b = a;
        b.translation[0] += 1e-6;
        assert!(!a.differs(&b));
        b.translation[0] += 0.01;
        assert!(a.differs(&b));
    }

    #[test]
    fn args_round_trip_preserves_types() {
        let mut args = HashMap::new();
        args.insert("count".into(), ScriptActionValue::Int(7));
        args.insert("speed".into(), ScriptActionValue::Float(1.5));
        args.insert("alive".into(), ScriptActionValue::Bool(true));
        args.insert("name".into(), ScriptActionValue::String("bob".into()));
        args.insert("pos".into(), ScriptActionValue::Vec3([1.0, 2.0, 3.0]));
        let back = args_from_bytes(&args_to_bytes(&args));
        assert_eq!(back.len(), 5);
        assert_eq!(back["count"], ScriptActionValue::Int(7));
        assert_eq!(back["speed"], ScriptActionValue::Float(1.5));
        assert_eq!(back["pos"], ScriptActionValue::Vec3([1.0, 2.0, 3.0]));
    }

    #[test]
    fn garbage_decodes_to_nothing() {
        assert!(args_from_bytes(b"not json").is_empty());
        assert_eq!(decode(&[0xff; 8]), None);
    }
}
