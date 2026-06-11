# State Replication

How the server keeps networked entities in sync on every client, built on Lightyear over native UDP.

## How replication works

Renzora's multiplayer is built on **Lightyear 0.26**, configured with the `udp` + `netcode` features only. The whole `renzora_network` crate is **native-only** — on the WebGPU/`wasm32` export it compiles to a no-op stub, so replication does not run in the browser build.

Replication is **server-authoritative and opt-in**:

- The **server** (`renzora --server`) or **host** (`renzora --host`, a windowed listen-server) owns the simulation.
- A plain windowed launch is a **client**. It connects dynamically (see [Connecting](#connecting)).
- An entity only replicates if you tag it with the **`Networked`** marker component. Nothing replicates by default.

> ⚠️ There is **no generic component replication and no mesh replication.** The protocol registers a fixed, small set of types (below). You cannot mark an arbitrary component for sync — `app.replicate::<MyComponent>()` does not exist in this engine.

### What gets replicated

`register_protocol` (in `renzora_network/src/protocol.rs`) registers exactly these replicated components, server → client:

| Component | Purpose |
|---|---|
| `Networked` | Marker that opts the entity into replication |
| `NetworkId(u64)` | Server-assigned network-wide id for the entity |
| `Name` | The entity's name |
| `NetworkPlayer` | Marker for a server-spawned player avatar |
| `NetworkOwner(OwnerKind)` | Who owns the entity (`Server` or `Client(id)`) |
| `Transform` | Position / rotation / scale, with linear interpolation |

Plus two channels — **Reliable** (ordered) and **Unreliable** (unordered), both bidirectional — and four message events: `SpawnRequest` / `DespawnRequest` (client → server) and `ChatMessage` / `GameEvent` (bidirectional). `GameEvent` is what RPCs ride on.

## The `Networked` marker

Add `Networked` to any entity that should be synchronized. You can add it in the editor's Inspector (it's a reflected component and serializes into `.ron` scenes), or from a Rust plugin:

```rust
use bevy::prelude::*;
use renzora_network::{Networked, NetworkTransform};

fn spawn_crate(mut commands: Commands) {
    commands.spawn((
        Name::new("Crate"),
        Transform::from_xyz(0.0, 1.0, 0.0),
        Networked,                  // opt in to replication
        NetworkTransform::default(),
    ));
}
```

On the **server**, the moment an entity gains `Networked`, the `auto_replicate_networked` system inserts Lightyear's `Replicate::to_clients(All)` and (unless interpolation is disabled) `InterpolationTarget::to_clients(All)`. A separate system assigns it a `NetworkId`. `Transform` then replicates because it is registered in the protocol.

> There is no Lua/Rhai function to mark an entity `Networked`. Replication is configured in the scene/Inspector or in engine code; scripts handle the *events* (RPCs) and gate server logic with `net_is_server()`.

## Tuning replication with `NetworkTransform`

`NetworkTransform` is an optional component that tunes how a `Networked` entity's `Transform` is sent. A `Networked` entity without one uses the defaults.

| Field | Default | Effect |
|---|---|---|
| `interpolate` | `true` | Smoothly interpolate between snapshots on remote peers. `false` = snap to each received position |
| `sync_rotation` | `true` | **Inert** — see below |
| `sync_scale` | `false` | **Inert** — see below |

> ⚠️ Only **`interpolate`** is actually read by the engine today. `sync_rotation` and `sync_scale` are **inert**: `Transform` always replicates wholesale (translation, rotation, and scale together). Don't rely on them to trim what's sent.

## Interpolation

Remote peers don't own an entity, so they render it **between the two most recent server snapshots** rather than snapping at the tick rate. This hides network jitter and makes motion look smooth.

This is wired by `register_protocol`, which registers `Transform` with `TransformLinearInterpolation::lerp` (translation lerp + rotation slerp). Entities the server marks `InterpolationTarget` get the interpolation systems; that's everything tagged `Networked` unless you set `interpolate = false` on its `NetworkTransform`.

> ⚠️ **There is no client-side prediction, reconciliation, or lag compensation.** `prediction.rs` exists but is inert (`smooth_correction` does nothing, `SNAP_THRESHOLD` is unused). Remote entities are interpolated-only; there is no rollback and no rewind-for-hit-detection. Likewise there is no delta compression, priority, relevancy culling, or quantization layer — `Transform` is replicated in full each time it changes.

## Ownership

Every networked entity carries a `NetworkOwner`, which wraps an `OwnerKind`:

```rust
pub enum OwnerKind {
    Server,        // NPCs, world objects (the default)
    Client(u64),   // owned by a specific connected client
}
```

`NetworkOwner` defaults to `Server`. The server assigns `Client(id)` to a player's avatar when that client connects. Ownership is **descriptive metadata that replicates** — the engine does not yet enforce authority based on it, so write server-authoritative logic explicitly and gate it with `net_is_server()`.

### Player avatars (`NetworkPlayer`)

`NetworkPlayer` is a marker meant for one server-spawned avatar per connected client; it's registered so clients receive it (and the owner) and can attach a visual via `Added<NetworkPlayer>`.

> ⚠️ Avatar spawning is **not implemented yet.** Nothing in the engine spawns a `NetworkPlayer` on join, and no client system reacts to `Added<NetworkPlayer>`. The component is in the protocol so game code can build on it, but the engine does not give you a player avatar out of the box.

### Replicated entities are *new* entities on the client

Lightyear replicates by creating a **brand-new entity on each client** that carries the replicated components (including the interpolated `Transform`). It does **not** map a `NetworkId` onto a pre-existing scene entity.

> ⚠️ A common gotcha: if both the server and clients already have the same object placed in a scene, the client's replicated copy is a *separate* entity. The `Transform` data syncs onto that new entity — but it has **no `Mesh`**, so by default you won't see it move. Replication transports data, not your scene's rendered objects.

## RPCs for events

For one-off events (a hit, a chat line, a score change), use **RPCs**. The script-facing call is `rpc(name, args)`:

```lua
-- client: ask the server to apply damage
function on_update()
    if is_key_just_pressed("E") then
        rpc("damage", { amount = 25 })
    end
end

-- server: receive it, validate, and broadcast a result
function on_rpc(name, args, from)
    if name == "damage" and net_is_server() then
        -- from == the sender's peer id (server-side only; see warning)
        rpc("damage_applied", { amount = args.amount })
    end
end
```

Under the hood, `rpc(name, args)` becomes a `net_rpc` `ScriptAction`, is serialized to a `GameEvent` JSON payload, and is sent on the **Reliable** channel. A client sends to the server; the server broadcasts to all clients. On receipt the event is delivered to local scripts via `on_rpc(name, args, from)`, and on the server it is **relayed to every other client** (no self-echo) — giving a client → server → clients fan-out.

> ⚠️ **The origin peer id is lost through relay.** When the server relays a client's RPC to the other clients, the event arrives with `from = server`, and the engine maps the server/raw peer to `0`. So relayed clients always see `from = 0`. Only the **server itself** sees the true sender id in `on_rpc`. Don't use `from` for client-to-client identification.

RPCs (and all networking) are **Lua-only**. Rhai is a subset backend with no networking functions, and the `on_rpc` / `on_player_joined` / `on_player_left` hooks fire only in Lua scripts.

### Network status and lifecycle (Lua)

| Function / hook | Notes |
|---|---|
| `net_is_server()` | True on the dedicated/host server |
| `net_is_client()` | True when connected and not the server |
| `net_is_connected()` | Connected (client) or running (server) |
| `net_player_count()` | Connected client count (server only; `0` elsewhere) |
| `rpc(name, args)` | Send an RPC (client → server, or server → all) |
| `on_rpc(name, args, from)` | Receive an RPC |
| `on_player_joined(id)` / `on_player_left(id)` | Fire **only on server/host**, with the real peer id |

See the [Scripting overview](../scripting/overview) for how these hooks fit the wider lifecycle.

## Connecting

There is no bare `net_connect` global. Connection is driven through the generic `action()` channel:

```lua
action("net_connect", { address = "127.0.0.1", port = 7636 })
-- ...later:
action("net_disconnect")
```

> The handshake is **insecure** (a zero private key, protocol id 0) — it is for LAN/dev only.

Server/connection defaults come from `[network]` in `project.toml`, overlaid by the CLI flags `--port` / `--addr` / `--tick-rate` / `--max-clients`:

```toml
[network]
server_addr = "127.0.0.1"
port        = 7636
transport   = "udp"   # only "udp" works (see note)
tick_rate   = 64
max_clients = 32
```

> ⚠️ `transport` accepts `udp`, `webtransport`, and `websocket`, but **only UDP is wired up.** The value is parsed and shown in the editor, yet no code selects a transport — Lightyear is compiled with the UDP feature only. `webtransport`/`websocket` are placeholders.

## Editor panels

`renzora_network_editor` adds Inspector cards for `Networked` and `NetworkTransform`, plus three native panels:

- **Network Monitor** — connection state and client list.
- **Network Entities** — replicated entities.
- **Network Settings** — read-only; edit `[network]` in `project.toml` (no connect/host buttons yet).

> ⚠️ The Network Monitor's RTT, jitter, packet-loss, and Client ID rows are **always zero/empty** — those `NetworkStatus` fields are never populated from Lightyear. Treat them as not-yet-implemented, not as a live readout.

## What's not implemented yet

To set expectations honestly, the following are **defined but stubbed or absent** in this alpha:

- Client input replication (`PlayerInput` exists but is unregistered).
- Client-side prediction / rollback / reconciliation (`prediction.rs` is inert).
- Lag compensation / server rewind.
- Automatic player-avatar spawning on `NetworkPlayer`.
- Generic component replication, mesh replication, and `NetworkId`-to-scene-entity mapping.
- Bandwidth features: delta compression, priority, relevancy/interest management, quantization.
- Authority transfer, alternate transports (WebTransport/WebSocket), networked physics, and security.
- `net_send` / `net_send_message` / `net_spawn` script actions (registered as TODO stubs that never reach the wire); `net_host_server` only logs "run with `--server`".

What *does* work today: dedicated/host server modes, the fixed replicated-component protocol, tick sync, server-authoritative `Transform` replication with snapshot interpolation, RPCs (with the relay caveat above), and server-side join/leave lifecycle hooks.
