# Multiplayer Overview

Renzora's multiplayer is its own: a UDP transport written for the engine, server-authoritative replication of the entities you mark, RPCs between peers, and an avatar for every player who joins. It lives in `renzora_network`, and a game reaches it as `renzora::network`.

> **Alpha status.** It works end to end (dedicated and listen servers, replication, RPCs, avatars, late joiners), and it is LAN-grade: traffic is **not encrypted** and there is no client-side prediction yet. Do not carry secrets over it, and expect a remote player's movement to be about 100 ms behind.

## The model in one paragraph

One process is the **server**. It runs the game rules and owns the truth. Everyone else is a **client**: it loads the same project, joins, and is sent every entity you marked `Networked`, kept up to date as the server changes them. A player's own avatar is the exception: its owner moves it, so it responds instantly, and the server passes those moves on. Anything else a client wants, it asks the server for with an RPC.

## Roles

There is one binary. A flag picks the role:

| Launch | Role | Window |
|---|---|---|
| `renzora` | Client (offline until it connects) | yes |
| `renzora --connect 10.0.0.5:7636` | Client that joins at startup | yes |
| `renzora --host` | **Listen server**: the server plus a local player | yes |
| `renzora --server` | **Dedicated server**: headless, no player of its own | no |

An exported game takes the same flags. See [Server Setup](server-setup).

## Testing from the editor

Set the session up once in `project.toml`, then press **Play**:

```toml
[network]
port = 7636
play_mode = "host"   # "single" (default), "host" or "server"
play_clients = 1     # extra client windows to open beside the game
```

- `host`: the game runs as a listen server, and `play_clients` more windows join it.
- `server`: a headless dedicated server starts first (its log appears in the Console under **Server**), then the game and the extra clients join it.

**Stop**, or closing the game window, ends every process in the session. The editor's live link follows the main game window only.

## Marking things for multiplayer

| Component | What it does |
|---|---|
| `Networked` | Replicate this entity to every client. |
| `NetworkTransform` | How its transform replicates: who moves it (**Server** or **Owner**), interpolation, rotation and scale. |
| `NetworkPlayerSpawner` | Give every player who joins an instance of a prefab, owned by them. |
| `NetworkPrefab` | What a networked entity looks like: a prefab every peer instances locally. |
| `NetworkOwner` | Whose it is. Set by the server; shown read-only in the Inspector. |

All of them are in the Inspector under **Networking**. An entity placed in the scene and marked `Networked` is the **same** entity on every peer: the client matches it to its own copy by scene id rather than spawning a second one.

## A first multiplayer game

1. Make a player prefab (a `.bsn` scene) with a mesh and a script.
2. Put an entity with a **Player Spawner** in your level, and point it at the prefab.
3. Set `play_mode = "host"` and `play_clients = 1`, and press Play.

Two windows open, each with two avatars. The script on the prefab moves only the avatar it belongs to:

```rust
use bevy::prelude::*;
use renzora::network as net;
use renzora::ScriptCtx;

fn update(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    let dt = ctx.delta();
    let world = ctx.world();
    // The networked entity is the prefab instance above this script.
    let Some(avatar) = net::networked_root(world, me) else { return };
    if !net::is_mine(world, avatar) {
        return; // someone else's: the network moves it
    }
    let right = world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|k| k.pressed(KeyCode::KeyD));
    if right {
        if let Some(mut t) = world.get_mut::<Transform>(avatar) {
            t.translation.x += 5.0 * dt;
        }
    }
}

renzora::script!(update);
```

The spawner gives avatars a `NetworkTransform` whose authority is **Owner**, which is what lets the owner's moves through.

## Who runs what: `has_authority`

Every peer runs every script. Game logic must run once, on the right peer:

```rust
if !renzora::network::has_authority(world, entity) {
    return;
}
```

`has_authority` is true on the server for everything it simulates, true on a client for its own owner-moved avatar, and true for everything when there is no session. That last part means a multiplayer scene still plays single-player with no second code path.

| Call | Answers |
|---|---|
| `net::is_server(world)` | This process is the server (dedicated or host). |
| `net::is_client(world)` | This process is a client. |
| `net::is_connected(world)` | There is a live session. |
| `net::local_peer_id(world)` | This peer's id. The host is `1`, a dedicated server `0`. |
| `net::players(world)` | Everyone in the session. |
| `net::is_mine(world, e)` | `e` (or its networked ancestor) belongs to this peer's player. |
| `net::has_authority(world, e)` | This peer should simulate `e`. |
| `net::owner_of(world, e)` / `net::networked_root(world, e)` | Ownership and the networked ancestor of a prefab's child. |

## RPCs

An RPC is a name and some arguments, sent to one peer or many:

```rust
use renzora::network::{Rpc, RpcTarget};

// A client asks the server.
Rpc::new("pickup").arg("coin", coin_id).to(RpcTarget::Server).send(world);

// The server tells everyone.
Rpc::new("scored").arg("player", who).arg("points", 3).to(RpcTarget::All).send(world);
```

| Target | Goes to |
|---|---|
| `Server` | the server only |
| `All` | every peer, the sender and the server included |
| `Others` (default) | every peer but the sender |
| `Peer(id)` | one peer |

RPCs are reliable and ordered unless you call `.unreliable()`. They arrive as a hook, with the sender's real id (stamped by the server, so a client cannot speak as someone else):

```rust
use renzora::network::RpcArgs;
use renzora::{ScriptCtx, ScriptHook};

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Rpc { name: "scored", args, from } => {
            let points = args.i64("points").unwrap_or(0);
            info!("{from} says: +{points}");
        }
        ScriptHook::PlayerJoined { id } => info!("{id} joined"), // server only
        _ => {}
    }
}
```

With no session, an RPC to `Server` or `All` is delivered to this process's own scripts, so "ask the server" still works single-player.

## Spawning shared things

Only the server creates shared state. It spawns a networked prefab, and every client gets it:

```rust
use renzora::network::{self as net, OwnerKind};

if net::is_server(world) {
    net::spawn_prefab(world, "prefabs/coin.bsn", Transform::from_xyz(3.0, 0.5, 0.0), OwnerKind::Server);
}
```

Despawn it on the server and it is despawned everywhere. A client calling `spawn_prefab` gets `None`: it should ask the server with an RPC.

## Connecting

```rust
renzora::network::connect(world, "10.0.0.5", 7636); // "" and 0 take [network] server_addr / port
renzora::network::disconnect(world);
```

Or launch with `--connect [address[:port]]`. A `--connect` launch keeps trying for a minute if the server is not up yet.

## Other languages

A language plugin reaches the same system through its verbs: `rpc(name, args)` (add `__to = "server" | "all" | "others" | <peer id>` and `__unreliable = true` to the arguments), `action("net_connect", { address, port })`, `action("net_disconnect")`, `action("net_spawn", { prefab, x, y, z })` (server only), and the `on_rpc(name, args, from)`, `on_player_joined(id)` and `on_player_left(id)` hooks.

## Editor panels

**Network Monitor**, **Network Entities** and **Network Settings** show the editor's own view of a session. Since Play runs the game in its own process, watch a running session in the game's HUD or its log in the Console (categories **Runtime**, **Server** and **Client**).

## Not built yet

- Client-side prediction and reconciliation (a server-moved entity a client controls has a round trip of delay; owner-moved avatars do not).
- Interest management: every client is sent every networked entity.
- Encryption, and transports other than UDP. There is no browser multiplayer: on the web, `renzora::network` behaves as it does offline.
- Networked physics: a dynamic rigid body replicated to a client is still simulated there. Make client copies kinematic, or move such things by script on the server.

## Related

- [Server Setup](server-setup): flags, `[network]`, running a dedicated server.
- [State Replication](replication): what replicates, when, and how to replicate your own components.
