//! RPCs with no session: a multiplayer game played alone.
//!
//! A game written for multiplayer asks the server to do things: "I picked this
//! coin up", sent to `RpcTarget::Server`. With nobody connected this process is
//! as much the server as there is, so those calls are delivered to its own
//! scripts and the game plays single-player without a second code path.
//! What is addressed only to others goes nowhere, because there are none.

use bevy::prelude::*;
use renzora_network_api::{NetOutbox, RpcTarget};
use renzora_scripting_api::{IncomingRpc, ScriptRpcInbox};

/// Deliver queued RPCs locally when there is neither a server nor a client.
pub fn route_offline(world: &mut World) {
    if world.contains_resource::<crate::server::NetServer>() || world.contains_resource::<crate::client::NetClient>() {
        return;
    }
    let rpcs = std::mem::take(&mut world.resource_mut::<NetOutbox>().rpcs);
    if rpcs.is_empty() {
        return;
    }
    let mut inbox = world.resource_mut::<ScriptRpcInbox>();
    for rpc in rpcs {
        if matches!(rpc.target, RpcTarget::Server | RpcTarget::All) {
            inbox.pending.push(IncomingRpc { name: rpc.name, args: rpc.args, from: 0 });
        }
    }
}
