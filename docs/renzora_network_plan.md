# Renzora Networking Plan — full Lightyear coverage

Goal: expose **every Lightyear 0.26 capability** through the engine's own
surface — **components, scripting verbs/hooks, and editor UI** — so games build
multiplayer by composing primitives. **No game content (avatars, movement,
spawning) is ever hardcoded in engine source.** The engine provides the tools;
scripts/scenes decide the behavior.

Status legend: ✅ done · 🟡 partial · ⬜ todo

Lightyear feature → phase coverage map at the bottom.

---

## Phase 0 — Foundation (transport, connection, protocol) ✅🟡
The plumbing everything rides on.
- ✅ UDP transport, netcode connect/disconnect, dedicated server (`--server`, headless), client setup.
- ✅ Protocol: reliable + unreliable channels (with `add_direction`), message **triggers** (`GameEvent`, `ChatMessage`, `SpawnRequest`, `DespawnRequest`).
- ✅ Tick/time sync (via `ClientPlugins`/`ServerPlugins`).
- ✅ Editor: Network Monitor / Entities / Settings panels.
- 🟡 Insecure handshake (protocol_id 0, zero key) — fine for LAN/dev; see Phase 12 (security).

## Phase 1 — Session & connection primitives (scripting) 🟡
Let scripts reason about the connection. The seam authoritative logic needs.
- ✅ `net_is_server()`, `net_is_client()`, `net_is_connected()`, `net_player_count()` getters (read `NetworkBridge` → script context). Demo: `assets/scripts/net_score.lua`.
- ⬜ Connection lifecycle hooks: `on_connected()`, `on_disconnected(reason)` (client side).
- ✅ `net_connect(addr, port)` / `net_disconnect()` verbs.
- ✅ **Host-server mode** (Lightyear `HostClient`) — one process = server + a local client (no UDP for the local player). DONE 2026-05-27: `renzora-runtime --host` sets a `HostServer` marker (windowed, not headless); `NetworkPlugin` adds the client half while `NetworkServerPlugin` owns the protocol/observers (registered exactly once, after both plugin sets) and spawns a local `(Client, LinkOf { server })` once the server starts → lightyear's observers promote it to `HostClient`. Recipe validated by `crates/renzora_network/tests/host_server.rs` (incl. with the real protocol registered).
- ⬜ Editor: connect/host buttons in the Network panel (not just project.toml).

## Phase 2 — Messaging & RPCs ✅🟡
Discrete "this happened" events. Mostly done.
- ✅ `rpc(name, args)` → `on_rpc(name, args, from)`, broadcast, server relay, no echo.
- ⬜ Targeted RPC: `rpc_to(peer, name, args)` and `rpc_to_server(...)` (NetworkTarget).
- ⬜ Reliable vs unreliable per call (`rpc(name, args, { channel = "unreliable" })`).
- ⬜ Preserve **origin peer id** through server relay (currently shows `from 0`).
- ⬜ Typed message hooks: wire `ChatMessage`/`SpawnRequest` and a generic `on_message`.

## Phase 3 — State replication via components 🟡
Continuous "where things are." The synchronizer half.
- ✅ `Networked` marker → server-authoritative `Replicate` + `InterpolationTarget`.
- ✅ `Transform` replication with linear interpolation; `NetworkTransform` tuning (interpolate / rotation / scale); `NetworkOwner`, `NetworkPlayer`, `NetworkId`; inspector cards.
- ⬜ **Generic component replication**: a way to mark *any* registered component to replicate (e.g. `NetworkedComponents` list, or per-type opt-in) — not just Transform.
- ⬜ **Replicate script variables**: `sync_var("health", ...)` so script state syncs.
- ⬜ **Delta compression** (Lightyear `Diffable`) for bandwidth on large/often-changing components.
- ⬜ Per-component send-rate / change-detection config on `NetworkTransform`.

## Phase 4 — Player lifecycle, ownership & spawning 🟡
The MultiplayerSpawner equivalent — entirely script/prefab-driven.
- ✅ Server hooks: `on_player_joined(id)`, `on_player_left(id)` — server tracks real lightyear peer ids on connect/disconnect (`ScriptNetLifecycleInbox` in core), dispatched to scripts via the same path as `on_rpc`.
- ⬜ `spawn_networked(prefab_or_primitive, x, y, z, owner)` verb → spawns `Networked` + `NetworkOwner`.
- ⬜ **Prefab-spawn replication**: server says "spawn prefab P as net id N owned by C"; each client instantiates P locally **with its own mesh/visual** (solves "meshes don't replicate"). Lightyear `PreSpawned` for client-predicted spawns.
- ⬜ `Controlled` / `ControlledBy` — which entity a client owns (so a script knows "this avatar is mine").
- ⬜ Despawn-on-disconnect cleanup of a player's owned entities (opt-in).

## Phase 5 — Client input ⬜
Client → server input, the basis of authoritative movement. (Lightyear `inputs`.)
- ⬜ `input_native` backend: register a `PlayerInput` message, buffer per tick, resend last N for packet loss.
- ⬜ Bridge to the engine `InputMap` (actions) so scripts read the same actions client+server.
- ⬜ Optional backends: `leafwing` (leafwing-input-manager), `input_bei` (bevy_enhanced_input).
- ⬜ Script surface: input flows to the server; server scripts move owned entities using it.

## Phase 6 — Client-side prediction & rollback ⬜
"Your own avatar feels instant." (Lightyear `prediction`.)
- ⬜ `PredictionTarget` on a client's owned entity; predict from local input, reconcile on server snapshot.
- ⬜ Rollback + re-simulation; `enable_correction` for smooth error correction.
- ⬜ Prediction config on `NetworkTransform` (predicted vs interpolated per entity).
- ⬜ `PreSpawned` predicted entity spawning (shoot a projectile instantly, reconcile with server).

## Phase 7 — Interpolation polish ⬜🟡
Smoothness for non-owned entities. (Lightyear `interpolation`, `frame_interpolation`.)
- ✅ Snapshot interpolation for `Transform`.
- ⬜ `frame_interpolation` — smooth render between fixed-update ticks.
- ⬜ Interpolation delay / snapshot-buffer tuning exposed on `NetworkTransform`.
- ⬜ Custom interpolation for game components (not just Transform).

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
- ✅ UDP (native).
- ⬜ **WebTransport** + **WebSocket** → browser/WASM clients (`renzora_network` is currently a WASM no-op).
- ⬜ **Steam** sockets (Steam friends/lobbies transport).
- ⬜ **Crossbeam** in-memory transport — for host-server and **headless integration tests**.
- ⬜ Config-driven selection via existing `TransportKind` enum (udp/webtransport/websocket) + project.toml.

## Phase 11 — Networked physics ⬜
Predicted/replicated rigid bodies. (Lightyear `avian2d`/`avian3d`.)
- ⬜ Integrate with `renzora_physics` (Avian backend already present): replicate + predict bodies, server-authoritative physics with client prediction.
- ⬜ A `NetworkedPhysics` marker / config tying a body into the prediction set.

## Phase 12 — Security & robustness ⬜
- ⬜ Secure netcode: real `protocol_id` + private key (crypto connect tokens) instead of zeros; token server / auth flow.
- ⬜ Auto-reconnect: retry while `Disconnected`, guard double-`Connecting`, transient-drop recovery.
- ⬜ Bandwidth/priority limiting per channel; replication send budgets.

## Phase 13 — Deterministic lockstep (alternative mode) ⬜
For RTS/fighting games. (Lightyear `deterministic`.)
- ⬜ Inputs-only replication + deterministic simulation (no state replication), with desync detection.

## Phase 14 — Diagnostics, tooling & tests ⬜🟡
- 🟡 Editor panels (Monitor/Entities/Settings) — expand with RTT/bandwidth graphs, replication inspector, per-entity owner/authority view.
- ⬜ Lightyear `metrics` + `debug` (lightyear_ui) overlay wired into the editor.
- ⬜ **Headless integration tests** via crossbeam transport (server+client apps stepped in lockstep) — RPC delivery, replication convergence, spawn/despawn. (CI-only on Windows due to dll link cap.)

## Phase 15 — Session layer (above Lightyear) ⬜
- ⬜ Lobby/matchmaking, room browser, ready-up, player list — built on the messaging + host-server primitives.

---

## Cross-cutting principles
- Every capability is reachable from **scripting (verbs + hooks)** and/or **components** with **editor UI**; nothing game-specific is hardcoded in engine crates.
- Server-authoritative by default; authority is explicit (`NetworkOwner`, Phase 9).
- WASM/headless paths must keep compiling (feature-gated).

## Lightyear feature → phase
| Lightyear feature | Phase |
|---|---|
| udp | 0 ✅ |
| netcode | 0 ✅ / 12 (secure) |
| client, server | 0 ✅ |
| host-server (HostClient) | 1 |
| messages/triggers | 0 ✅ / 2 |
| replication | 3 |
| delta (Diffable) | 3 |
| hierarchy replication | 4 |
| prespawn (PreSpawned) | 4, 6 |
| controlled / controlled_by | 4 |
| inputs / input_native / input_bei / leafwing | 5 |
| prediction | 6 |
| interpolation | 7 ✅🟡 |
| frame_interpolation | 7 |
| visibility / rooms | 8 |
| authority | 9 |
| websocket / webtransport | 10 |
| steam | 10 |
| crossbeam | 10, 14 |
| avian2d / avian3d | 11 |
| sync | 0 ✅ |
| metrics / debug / trace / ui | 14 |
| deterministic | 13 |
| std / web | 10 |
