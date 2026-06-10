# Renzora Networking Plan — full Lightyear coverage

A roadmap for exposing Lightyear's multiplayer feature set through the engine's own surface (components, scripting verbs/hooks, editor UI), tracked against what is actually compiled and wired today.

The goal is that games build multiplayer by composing engine primitives. **No game content (avatars, movement, spawning) is hardcoded in engine source** — the engine provides the tools; scripts and scenes decide the behavior.

Status legend: ✅ done · 🟡 partial · ⬜ todo

The Lightyear feature → phase coverage map is at the bottom.

> ⚠️ **Reality check.** "Expose every Lightyear 0.26 capability" is the aspiration, not the current state. Only **Phase 0**, the **RPC core of Phase 2**, **host mode (Phase 1)**, and **basic `Transform` replication + interpolation (Phase 3/7)** are implemented. Everything else below is unimplemented or stub-only. Several knobs and panels exist in the UI but are inert (called out per-phase).

---

## Compiled-in reality (read this first)

The networking crate is `renzora_network`. Lightyear is **0.26.4**, pulled with only the **`udp`** and **`netcode`** features, and **only on native targets**:

```toml
# crates/renzora_network/Cargo.toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
lightyear = { version = "0.26", features = ["udp", "netcode"] }
```

- **WASM has no networking.** On `wasm32` the whole crate compiles to a no-op stub.
- **UDP is the only transport** that can ever work, because the WebSocket/WebTransport Lightyear features are not compiled in (see Phase 10).
- **The binary is `renzora` / `renzora.exe`.** There is one workspace binary; networking modes are selected by flag at runtime, not by a separate `renzora-runtime` executable. (`renzora-runtime` only appears in exported-template scripts, not when running locally.)

### Modes (selected in `src/main.rs`)

| Launch | Marker | Rendering | Plugins |
|---|---|---|---|
| (default windowed) | — | full | client half only; connects on demand via `PendingNetworkConnect` |
| `--server` | `DedicatedServer` | **headless** (no GPU/window, `ScheduleRunnerPlugin` at the net tick) | `NetworkServerPlugin` |
| `--host` | `HostServer` | full (windowed listen server) | client half **+** `NetworkServerPlugin` |

`--host` wins if both `--host` and `--server` are passed. A server/host launch is **never** an editor session. Scripts **do** run on the headless dedicated server, so server-authoritative scripts execute server-side.

### Config & defaults

CLI flags `--port`, `--addr` / `--address`, `--tick-rate`, `--max-clients` overlay the `[network]` table in `project.toml`. Defaults (`config.rs`):

| Setting | Default |
|---|---|
| port | **7636** |
| tick_rate | **64** |
| max_clients | **32** |

```bash
# dedicated headless server on the defaults
renzora --server

# windowed listen server (client + server in one process)
renzora --host --port 7636 --max-clients 32
```

> ⚠️ **The handshake is insecure.** Authentication is `Manual { private_key = [0u8; 32], protocol_id = 0 }` — fine for LAN/dev only. Real connect tokens are Phase 12.

### Scripting surface that actually exists today

```lua
-- read-only getters (bare Lua globals)
net_is_server()      -- bool
net_is_client()      -- bool
net_is_connected()   -- bool
net_player_count()   -- int

-- discrete events
rpc("damage", { amount = 25 })   -- broadcast through the server

-- connection control is routed through action(), NOT bare verbs:
action("net_connect", { address = "127.0.0.1", port = 7636 })
action("net_disconnect")
```

Hooks (**Lua only** — Rhai implements just `props`/`on_ready`/`on_update`):

```lua
function on_rpc(name, args, from) end      -- from is a peer id (see relay caveat below)
function on_player_joined(id) end          -- server/host only
function on_player_left(id) end            -- server/host only
```

> ⚠️ **Stubs that compile but never touch the wire** (`script_extension.rs`):
> - `action("net_send", ...)` / `action("net_send_message", ...)` — log a line, then `// TODO: send via Lightyear`.
> - `action("net_spawn", ...)` — logs a spawn request, then `// TODO: send SpawnRequest`.
> - `action("net_host_server", ...)` — only logs *"Run the runtime with `--server` for a dedicated server."*
>
> Do not treat these as working. The only verbs that do real work are `rpc(...)`, `net_connect`, and `net_disconnect`.

---

## Phase 0 — Foundation (transport, connection, protocol) ✅🟡
The plumbing everything rides on.
- ✅ UDP transport (`ServerUdpIo`/`UdpIo`, hardcoded), netcode connect/disconnect, dedicated server (`--server`, headless), client setup.
- ✅ Protocol (`protocol.rs`): Reliable (ordered) + Unreliable (unordered) channels, both Bidirectional; message events `GameEvent` / `ChatMessage` (Bidirectional), `SpawnRequest` / `DespawnRequest` (ClientToServer).
- ✅ Tick/time sync (via `ClientPlugins` / `ServerPlugins`).
- ✅ Editor: Network Monitor / Entities / Settings panels (`renzora_network_editor`).
- 🟡 Insecure handshake (`protocol_id = 0`, zero key) — LAN/dev only; see Phase 12.

## Phase 1 — Session & connection primitives (scripting) 🟡
Let scripts reason about the connection.
- ✅ `net_is_server()`, `net_is_client()`, `net_is_connected()`, `net_player_count()` getters (read `NetworkBridge` → script context). Demo: `assets/scripts/net_score.lua`.
- ✅ `action("net_connect", { address, port })` / `action("net_disconnect")` — routed through `action()` (they insert `PendingNetworkConnect` / `PendingNetworkDisconnect`). **Not** bare `net_connect()`/`net_disconnect()` verbs.
- ✅ **Host-server mode** (Lightyear `HostClient`) — one process = server + a local client (no UDP for the local player). `renzora --host` sets a `HostServer` marker (windowed); the client half plus `NetworkServerPlugin` register the protocol once, spawn a local `(Client, LinkOf { server })`, and Lightyear's observers promote it to a `HostClient` after a few frames. Validated by `crates/renzora_network/tests/host_server.rs` (including with the real protocol registered).
- ⬜ Connection lifecycle hooks: `on_connected()`, `on_disconnected(reason)` (client side).
- ⬜ Editor: connect/host buttons in the Network panel (settings panel is currently read-only — *"Edit [network] in project.toml"*).

## Phase 2 — Messaging & RPCs ✅🟡
Discrete "this happened" events. Core is done.
- ✅ `rpc(name, args)` → `net_rpc` `ScriptAction` → `PendingOutgoingRpc` → serialized as a `GameEvent` JSON payload on the **Reliable** channel → `on_rpc(name, args, from)`. Client→server; server relays to every **other** client (no self-echo).
- ⬜ Preserve **origin peer id** through server relay. Relayed `GameEvent`s arrive with `from = server`, and `peer_id_to_u64` maps Server/Raw to **0** — so only the server sees the true sender; **relayed clients always see `from = 0`**.
- ⬜ Targeted RPC: `rpc_to(peer, name, args)` and `rpc_to_server(...)` (NetworkTarget).
- ⬜ Reliable vs unreliable per call (`rpc(name, args, { channel = "unreliable" })`) — currently always Reliable.
- ⬜ Typed message hooks: wire `ChatMessage` / `SpawnRequest` and a generic `on_message`.

## Phase 3 — State replication via components 🟡
Continuous "where things are."
- ✅ `Networked` marker → `auto_replicate_networked` inserts `Replicate::to_clients(All)` + `InterpolationTarget` (server-authoritative).
- ✅ `Transform` replication with linear interpolation (`TransformLinearInterpolation::lerp`); `NetworkOwner`, `NetworkPlayer`, `NetworkId` registered; inspector cards for `Networked` + `NetworkTransform`.
- 🟡 `NetworkTransform` tuning — **only `interpolate` is read.** `sync_rotation` and `sync_scale` are **inert**: they exist on the struct and in the inspector but are never consulted, and `Transform` always replicates wholesale.
- ⬜ **Generic component replication**: a way to mark *any* registered component to replicate (e.g. a `NetworkedComponents` list, or per-type opt-in). Today only the fixed protocol set replicates — no `Mesh`, no arbitrary components.
- ⬜ **Replicate script variables**: `sync_var("health", ...)` so script state syncs.
- ⬜ **Delta compression** (Lightyear `Diffable`) for bandwidth.
- ⬜ Per-component send-rate / change-detection config on `NetworkTransform`.

> ⚠️ Replication creates a **new** client-side entity carrying the interpolated `Transform`; it does **not** map onto a pre-existing scene entity (no `NetworkId`-based entity mapping), and that entity has **no `Mesh`**, so it is invisible. Transform data syncs, but not visibly onto an existing cube. This is the root reason "meshes don't replicate" — solved by prefab-spawn in Phase 4.

## Phase 4 — Player lifecycle, ownership & spawning 🟡
The MultiplayerSpawner equivalent — entirely script/prefab-driven.
- ✅ Server hooks: `on_player_joined(id)`, `on_player_left(id)`. The server tracks real Lightyear peer ids in `handle_new_clients` / `handle_disconnects`, pushes `NetPlayerEvent` into `ScriptNetLifecycleInbox`, and dispatches to scripts via the same path as `on_rpc`. **Fire on server/host only.**
- ⬜ `spawn_networked(prefab_or_primitive, x, y, z, owner)` verb → spawns `Networked` + `NetworkOwner`. (`NetworkPlayer` is a registered replicated component, but **nothing spawns avatars on join and no client system reacts to `Added<NetworkPlayer>`** to give a visual.)
- ⬜ **Prefab-spawn replication**: server says "spawn prefab P as net id N owned by C"; each client instantiates P locally **with its own mesh/visual** (this is what fixes invisible replicated entities). Lightyear `PreSpawned` for client-predicted spawns.
- ⬜ `Controlled` / `ControlledBy` — which entity a client owns (so a script knows "this avatar is mine").
- ⬜ Despawn-on-disconnect cleanup of a player's owned entities (opt-in).

## Phase 5 — Client input ⬜
Client → server input, the basis of authoritative movement. (Lightyear `inputs`.)
- ⬜ `input_native` backend: register a `PlayerInput` message, buffer per tick, resend last N for packet loss. **`PlayerInput` is defined in `input.rs` but is not registered in the protocol or used anywhere** — Phase 5 is a stub.
- ⬜ Bridge to the engine `InputMap` (actions) so scripts read the same actions client + server.
- ⬜ Optional backends: `leafwing` (leafwing-input-manager), `input_bei` (bevy_enhanced_input).
- ⬜ Script surface: input flows to the server; server scripts move owned entities using it.

## Phase 6 — Client-side prediction & rollback ⬜
"Your own avatar feels instant." (Lightyear `prediction`.)
- ⬜ `PredictionTarget` on a client's owned entity; predict from local input, reconcile on server snapshot. **`prediction.rs` is inert** — `smooth_correction` does nothing and `SNAP_THRESHOLD` is unused.
- ⬜ Rollback + re-simulation; `enable_correction` for smooth error correction.
- ⬜ Prediction config on `NetworkTransform` (predicted vs interpolated per entity).
- ⬜ `PreSpawned` predicted entity spawning (shoot a projectile instantly, reconcile with server).

## Phase 7 — Interpolation polish ⬜🟡
Smoothness for non-owned entities. (Lightyear `interpolation`, `frame_interpolation`.)
- ✅ Snapshot interpolation for `Transform`.
- ⬜ `frame_interpolation` — smooth render between fixed-update ticks.
- ⬜ Interpolation delay / snapshot-buffer tuning exposed on `NetworkTransform`.
- ⬜ Custom interpolation for game components (not just `Transform`).

## Phase 8 — Interest management / visibility ⬜
Scale to large worlds — only replicate what each client cares about. (Lightyear `visibility`, rooms.)
- ⬜ `NetworkVisibility` / Rooms: group entities + clients into rooms; replicate per room.
- ⬜ Distance/zone-based interest (a `NetworkRelevance` component or volume).
- ⬜ Script verbs: `net_room_join(client, room)`, `net_room_add(entity, room)`.

## Phase 9 — Authority transfer ⬜
Dynamic ownership handoff. (Lightyear `authority`.)
- ⬜ `HasAuthority`, `RequestAuthority`, `GiveAuthority` exposed as verbs/components.
- ⬜ Use cases: client grabs a physics prop, server reclaims on release.

## Phase 10 — Transports & platforms ⬜🟡
Reach every platform. (Lightyear transports.)
- ✅ UDP (native, hardcoded).
- ⬜ **WebTransport** + **WebSocket** → browser/WASM clients. Blocked: those Lightyear features are **not compiled in** (only `udp` + `netcode`), and `renzora_network` is a WASM no-op stub.
- ⬜ **Steam** sockets (friends/lobbies transport).
- ⬜ **Crossbeam** in-memory transport — for host-server and headless integration tests.
- ⬜ Config-driven transport selection. The `TransportKind` enum (`udp`/`webtransport`/`websocket`) **is parsed from `project.toml` and shown in the editor settings panel, but no code selects a transport from it** — UDP is hardcoded. Wiring this up also requires the missing Lightyear features above.

> Unrelated: `websocket_plugin` (editor cdylib, tungstenite dev server on port 8080) is an editor remote-command channel, **not** this planned Phase-10 WebSocket transport.

## Phase 11 — Networked physics ⬜
Predicted/replicated rigid bodies. (Lightyear `avian2d`/`avian3d`.)
- ⬜ Integrate with `renzora_physics` (Avian backend present): replicate + predict bodies, server-authoritative physics with client prediction.
- ⬜ A `NetworkedPhysics` marker / config tying a body into the prediction set.

## Phase 12 — Security & robustness ⬜
- ⬜ Secure netcode: real `protocol_id` + private key (crypto connect tokens) instead of zeros; token server / auth flow.
- ⬜ Auto-reconnect: retry while `Disconnected`, guard double-`Connecting`, transient-drop recovery.
- ⬜ Bandwidth/priority limiting per channel; replication send budgets.

## Phase 13 — Deterministic lockstep (alternative mode) ⬜
For RTS/fighting games. (Lightyear `deterministic`.)
- ⬜ Inputs-only replication + deterministic simulation (no state replication), with desync detection.

## Phase 14 — Diagnostics, tooling & tests ⬜🟡
- 🟡 Editor panels (Monitor / Entities / Settings) exist, but the Monitor shows **static zeros**: `NetworkStatus.rtt_ms`, `jitter_ms`, `packet_loss`, `client_id` and `ConnectedClient.rtt_ms` are **never populated from Lightyear**, so RTT/Jitter/Packet Loss always read 0 and **Client ID never displays**. Real RTT/bandwidth graphs, a replication inspector, and per-entity owner/authority views are todo.
- ⬜ Lightyear `metrics` + `debug` (lightyear_ui) overlay wired into the editor.
- ⬜ **Headless integration tests** via crossbeam transport (server + client apps stepped in lockstep) — RPC delivery, replication convergence, spawn/despawn. (CI-only on Windows due to the dll link cap.)

## Phase 15 — Session layer (above Lightyear) ⬜
- ⬜ Lobby/matchmaking, room browser, ready-up, player list — built on the messaging + host-server primitives.

---

## Cross-cutting principles
- Every capability is reachable from **scripting (verbs + hooks)** and/or **components** with **editor UI**; nothing game-specific is hardcoded in engine crates.
- Server-authoritative by default; authority is explicit (`NetworkOwner`, Phase 9).
- WASM/headless paths must keep compiling (feature-gated; WASM networking is a no-op stub).
- The decoupling layer lives in `renzora/src/core/mod.rs`: `NetworkBridge`, `IncomingRpc`, `ScriptRpcInbox`, `NetPlayerEvent`, `ScriptNetLifecycleInbox`, and the `DedicatedServer` / `HostServer` markers — so `renzora_scripting` reads networking state without depending on `renzora_network`.

## Lightyear feature → phase
| Lightyear feature | Phase | Compiled? |
|---|---|---|
| udp | 0 ✅ | yes |
| netcode | 0 ✅ / 12 (secure) | yes |
| client, server | 0 ✅ | yes |
| host-server (HostClient) | 1 ✅ | yes |
| messages/triggers | 0 ✅ / 2 | yes |
| sync | 0 ✅ | yes |
| replication | 3 🟡 | yes |
| interpolation | 7 ✅🟡 | yes |
| delta (Diffable) | 3 | — |
| hierarchy replication | 4 | — |
| prespawn (PreSpawned) | 4, 6 | — |
| controlled / controlled_by | 4 | — |
| inputs / input_native / input_bei / leafwing | 5 | — |
| prediction | 6 | — |
| frame_interpolation | 7 | — |
| visibility / rooms | 8 | — |
| authority | 9 | — |
| websocket / webtransport | 10 | **no** (feature not enabled) |
| steam | 10 | **no** |
| crossbeam | 10, 14 | **no** |
| avian2d / avian3d | 11 | — |
| metrics / debug / trace / ui | 14 | — |
| deterministic | 13 | — |
| std / web | 10 | web = no-op stub |
