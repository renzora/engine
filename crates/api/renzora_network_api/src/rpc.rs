//! Remote procedure calls: a name and a bag of arguments, sent to one peer or
//! many, arriving as `ScriptHook::Rpc` (Rust) or `on_rpc` (other languages).
//!
//! The arguments are the same `ScriptActionValue` bag every script hook
//! carries, so a Rust script and a Lua script on two machines can talk to each
//! other: what one sends is exactly what the other's handler reads.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora_scripting_api::ScriptActionValue;

/// Who an RPC is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RpcTarget {
    /// The server only. A client asking the server to do something ("I
    /// picked this up", "fire") is this.
    Server,
    /// Every peer in the session, the sender and the server included. The
    /// server announcing something to everyone is this.
    All,
    /// Every peer except the sender. The default, and what `rpc()` always
    /// meant before targets existed.
    #[default]
    Others,
    /// One peer, by id.
    Peer(u64),
}

/// How an RPC travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Delivery {
    /// Arrives exactly once, in the order it was sent relative to every other
    /// reliable message. The default: almost every RPC is an event that must
    /// not be lost.
    #[default]
    Reliable,
    /// Sent once and may be lost or overtaken. Right for something re-sent
    /// every frame anyway, where a stale copy arriving late is worse than none.
    Unreliable,
}

/// One RPC waiting to go out. Drained by `renzora_network` each frame.
#[derive(Debug, Clone)]
pub struct OutgoingRpc {
    pub target: RpcTarget,
    pub delivery: Delivery,
    pub name: String,
    pub args: HashMap<String, ScriptActionValue>,
}

/// Something a script asked of the session, other than an RPC.
#[derive(Debug, Clone, PartialEq)]
pub enum NetRequest {
    /// Join a server. An empty address or a zero port takes the project's
    /// `[network]` value.
    Connect { address: String, port: u16 },
    /// Leave the session.
    Disconnect,
}

/// The queue scripts and plugins write and the network layer drains.
///
/// A resource rather than an event so that a call made before the network
/// plugin built (or in a build without one) waits instead of vanishing; in a
/// build with networking stripped nothing ever drains it, which is correct for
/// a single-player game.
#[derive(Resource, Default, Debug)]
pub struct NetOutbox {
    pub rpcs: Vec<OutgoingRpc>,
    pub requests: Vec<NetRequest>,
}

/// A value an RPC argument can be made from.
///
/// Its own trait because `From<f32> for ScriptActionValue` cannot be written
/// here: both types belong to other crates.
pub trait IntoRpcArg {
    fn into_rpc_arg(self) -> ScriptActionValue;
}

impl IntoRpcArg for ScriptActionValue {
    fn into_rpc_arg(self) -> ScriptActionValue {
        self
    }
}
impl IntoRpcArg for f32 {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Float(self)
    }
}
impl IntoRpcArg for i64 {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Int(self)
    }
}
impl IntoRpcArg for i32 {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Int(self as i64)
    }
}
/// A peer id or a network id. Carried as the bits of an `i64`, since that is
/// the only integer an argument holds, and read back with
/// [`RpcArgs::u64`], which reverses the cast exactly.
impl IntoRpcArg for u64 {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Int(self as i64)
    }
}
impl IntoRpcArg for bool {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Bool(self)
    }
}
impl IntoRpcArg for &str {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::String(self.to_string())
    }
}
impl IntoRpcArg for String {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::String(self)
    }
}
impl IntoRpcArg for Vec3 {
    fn into_rpc_arg(self) -> ScriptActionValue {
        ScriptActionValue::Vec3(self.to_array())
    }
}

/// An RPC being built.
///
/// ```ignore
/// Rpc::new("pickup").arg("coin", coin_id).to(RpcTarget::Server).send(ctx.world());
/// ```
#[derive(Debug, Clone)]
#[must_use = "an Rpc does nothing until `.send(world)`"]
pub struct Rpc(OutgoingRpc);

impl Rpc {
    pub fn new(name: impl Into<String>) -> Self {
        Self(OutgoingRpc {
            target: RpcTarget::default(),
            delivery: Delivery::default(),
            name: name.into(),
            args: HashMap::new(),
        })
    }

    pub fn to(mut self, target: RpcTarget) -> Self {
        self.0.target = target;
        self
    }

    pub fn unreliable(mut self) -> Self {
        self.0.delivery = Delivery::Unreliable;
        self
    }

    pub fn arg(mut self, key: impl Into<String>, value: impl IntoRpcArg) -> Self {
        self.0.args.insert(key.into(), value.into_rpc_arg());
        self
    }

    /// Queue it. Goes out at the end of this frame.
    pub fn send(self, world: &mut World) {
        world
            .get_resource_or_insert_with(NetOutbox::default)
            .rpcs
            .push(self.0);
    }

    pub fn into_outgoing(self) -> OutgoingRpc {
        self.0
    }
}

/// Typed reads from a received argument bag.
///
/// Lenient about numbers, because JSON on the wire cannot tell `2` from
/// `2.0`: an integer reads as a float and a whole float as an integer.
pub trait RpcArgs {
    fn f32(&self, key: &str) -> Option<f32>;
    fn i64(&self, key: &str) -> Option<i64>;
    fn u64(&self, key: &str) -> Option<u64>;
    fn bool(&self, key: &str) -> Option<bool>;
    fn str(&self, key: &str) -> Option<&str>;
    fn vec3(&self, key: &str) -> Option<Vec3>;
}

impl RpcArgs for HashMap<String, ScriptActionValue> {
    fn f32(&self, key: &str) -> Option<f32> {
        match self.get(key)? {
            ScriptActionValue::Float(f) => Some(*f),
            ScriptActionValue::Int(i) => Some(*i as f32),
            _ => None,
        }
    }
    fn i64(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            ScriptActionValue::Int(i) => Some(*i),
            ScriptActionValue::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }
    fn u64(&self, key: &str) -> Option<u64> {
        match self.get(key)? {
            ScriptActionValue::Int(i) => Some(*i as u64),
            _ => None,
        }
    }
    fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            ScriptActionValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
    fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            ScriptActionValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    fn vec3(&self, key: &str) -> Option<Vec3> {
        match self.get(key)? {
            ScriptActionValue::Vec3(v) => Some(Vec3::from_array(*v)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_rpc_lands_in_the_outbox() {
        let mut world = World::new();
        Rpc::new("hit").arg("damage", 5).arg("at", Vec3::X).to(RpcTarget::Server).send(&mut world);
        let outbox = world.resource::<NetOutbox>();
        assert_eq!(outbox.rpcs.len(), 1);
        let rpc = &outbox.rpcs[0];
        assert_eq!(rpc.target, RpcTarget::Server);
        assert_eq!(rpc.args.i64("damage"), Some(5));
        assert_eq!(rpc.args.vec3("at"), Some(Vec3::X));
    }

    #[test]
    fn a_peer_id_survives_the_trip_through_i64() {
        let id = u64::MAX - 7;
        let mut args = HashMap::new();
        args.insert("who".to_string(), id.into_rpc_arg());
        assert_eq!(args.u64("who"), Some(id));
    }

    #[test]
    fn numbers_read_either_way() {
        let mut args = HashMap::new();
        args.insert("a".to_string(), ScriptActionValue::Int(3));
        args.insert("b".to_string(), ScriptActionValue::Float(4.0));
        assert_eq!(args.f32("a"), Some(3.0));
        assert_eq!(args.i64("b"), Some(4));
    }
}
