# Networking Test Plan

A hands-on, in-engine checklist to manually verify the multiplayer that's actually built today, driven by scripts plus an on-screen HUD.

This covers the scenarios that work right now: connect, presence, chat, authoritative score, RPC relay, disconnect, host mode, and Transform sync. Everything here uses shipped primitives only; nothing is special-cased. The unbuilt pieces and the surprising behaviours are listed under [Known limitations](#known-limitations) at the end, so read that before you start judging results.

> Test-harness scripts live in `assets/scripts/`: `net_connect.lua`, `net_hud.lua`, `net_lobby.lua`, `net_chat.lua`, `net_score.lua`, `multiplayer_ping.lua`, `net_move.lua`. They are Lua (`.lua`); networking is a Lua-only surface (the Rhai backend has no `rpc`/`net_*`/`action` and no `on_rpc`/`on_player_*` hooks).

---

## 1. Build a binary

Build a game-runtime binary with the `renzora` CLI — every build runs inside the Docker toolchain (there is no native `cargo` build). The dedicated server is the **same `renzora` binary** launched with `--server`; there is no separate server executable.

```bash
# Game runtime (no editor bundle) -> dist/windows-x64/renzora.exe
renzora build windows
```

The same binary is the client, the server, and the host — the mode is chosen by a runtime flag, not by a build.

> **Editor vs game.** A bare launch boots the **editor** if `renzora_editor.dll` sits beside the exe, otherwise it boots the shipped **game**. `renzora build` writes a game runtime (no bundle) into `dist/windows-x64/`, so that binary is a game client. If you instead used `renzora run` (which builds the editor + bundle), pass `--no-editor` to launch a game client, or just press Play in the editor. A `--server`/`--host` launch is **never** an editor session, even when the bundle is present.

> **`renzora-runtime` is stale.** Some script header comments say `renzora-runtime --server`. The locally built binary is plain `renzora` / `renzora.exe`; `renzora-runtime` only applies to exported mobile/web templates. Use `renzora.exe --server`.

## 2. Build the HUD (one-time, in the editor)

Launch the editor (`renzora run`), add a **UI Canvas** named `HUD`, and inside it add five **Text** widgets named exactly:

| Widget name | Shows |
|---|---|
| `NetStatus` | role + connection (`SERVER - connected` / `CLIENT - offline`) |
| `Players`   | authoritative player count |
| `Events`    | last join/leave notice |
| `Chat`      | last chat line |
| `Score`     | server-authoritative score |

Scripts find widgets **by Name** (`action("ui_set_text", { name = ..., text = ... })`), so the names must match exactly (case-sensitive). If a widget is missing the matching `ui_set_text` is a harmless no-op, and every script also `print_log`s to the console — so you can run the whole plan HUD-free and just watch the consoles. The headless `--server` has no UI at all, so its HUD updates are no-ops there by design; read its console.

## 3. Attach the scripts

Put these on entities in your scene (an empty named `Net` is fine — several can share one entity):

- `net_connect.lua` — **exactly one** per client; set its `address` / `port` props (defaults `127.0.0.1` / `7636`). Connection is fired by `action("net_connect", { address, port })` in `on_ready`.
- `net_hud.lua`, `net_lobby.lua`, `net_chat.lua`, `net_score.lua`, `multiplayer_ping.lua` — attach the ones you want to exercise. Attach `net_lobby` to **one** entity only.
- `net_move.lua` — attach to a cube that exists in the scene on **every** peer, and add the `Networked` component to that cube in the inspector (this inserts Lightyear `Replicate::to_clients(All)` + an interpolation target). Read scenario #10 first — the visible result is not what you'd expect.

## 4. Launch

Default port **7636**, tick rate **64**, max clients **32**, overridable with `--port` / `--addr` / `--tick-rate` / `--max-clients`. Always start the server/host **first**.

**Option A — dedicated server + 2 clients (most realistic):**
```bash
./dist/windows-x64/renzora.exe --server   # console 1: headless server, port 7636
./dist/windows-x64/renzora.exe            # window 2: client A
./dist/windows-x64/renzora.exe            # window 3: client B
```

**Option B — host + 1 client (quick):**
```bash
./dist/windows-x64/renzora.exe --host     # window 1: you're server + a player
./dist/windows-x64/renzora.exe            # window 2: another player
```

`--host` is a windowed listen server (client + server in one process) and **wins over `--server`** if both are passed. The local host player runs as an in-process `HostClient` (no UDP loopback for itself).

> The binary name is platform-specific: `renzora.exe` on Windows, bare `renzora` on Linux/macOS. `renzora build` lands it in `dist/<platform>/` (e.g. `dist/windows-x64/`).

---

## 5. Scenario checklist

| # | Scenario | Steps | Expected |
|---|---|---|---|
| 1 | **Connect** | Start server, then a client (with `net_connect` + `net_hud`) | Client HUD `NetStatus` -> `CLIENT - connected`; server -> `SERVER - connected` |
| 2 | **Presence / join** | With `net_lobby` attached, connect a 2nd client | Server console: `[server] player <id> joined — N online`; all HUDs `Players: N` + `Events: player <id> joined`. The joined id is carried in the RPC payload (`args.who`), so clients show the **real** id here. |
| 3 | **Player count** | Connect/disconnect clients | `Players` rises/falls and matches the number of connected clients (authoritative on the server/host) |
| 4 | **RPC broadcast + no self-echo** | `multiplayer_ping` on all peers; press **P** on client A | A logs `sent ping` and does **not** echo to itself. The **server** (if the script is on its scene) logs `got ping from <A's real id>`. Client B logs `got ping from 0` — relayed RPCs lose the sender id (see Known limitations). |
| 5 | **RPC args + sender id** | `net_chat`; press **1/2/3** on a client | Sender sees `you: <text>` (local echo). Other clients' `Chat` -> `[player 0] <text>` — the sender id is **lost through relay**. Only the server sees the true sender. |
| 6 | **Server-authoritative score** | `net_score`; press **K** on a client | Only the server tallies; **all** `Score` labels jump to the same N. The server logs `player <id> scored — total N` with the **real** id (it receives `score_request` directly, not relayed). |
| 7 | **Server relay (client->client)** | Two clients + ping/chat; act on client A | Client **B** receives it, proving the server relays peer->peer (not just peer->server). It arrives with `from = 0`; the origin id is not preserved through the relay. |
| 8 | **Disconnect** | Close one client window | Server: `[server] player <id> left — N online`; remaining HUDs `Players` drops, `Events: player <id> left` |
| 9 | **Host mode** | Launch with `--host`, connect one client | Host HUD `SERVER - connected` and it counts as a player; score/chat work both ways |
| 10 | **Transform replication (data-only)** | `net_move` on a `Networked` scene cube | The server moves its copy in a circle and the `Transform` **stream** does replicate. But Lightyear spawns a **new, mesh-less, interpolated client entity** for it — it does **not** map onto the pre-existing scene cube (there is no `NetworkId` entity mapping) and the new entity has no `Mesh`, so it is **invisible**. The placed cube does **not** visibly move. Confirm replication via the **Network Entities** panel (entity count rises) or console logs, not by watching the cube. See Known limitations. |

---

## 6. Troubleshooting

- **Client won't connect:** start the server first; check the port matches (`--port` / `net_connect`'s `port` prop, default 7636); same machine uses `127.0.0.1`, LAN uses the server's IP; open UDP 7636 in the firewall. Only **native UDP** works.
- **Bare launch opens the editor instead of a game client:** `renzora_editor.dll` is beside the exe. Use a `renzora build` runtime build, delete the dll, or pass `--no-editor`.
- **HUD not updating:** the widget `Name` must match the script string exactly (case-sensitive). The headless `--server` has no UI — watch its console instead.
- **`on_player_joined` never fires on a client:** it is **server-only**. It runs only on the `--server`/`--host` process; clients learn presence from `net_lobby`'s broadcast `on_rpc("lobby", ...)`.
- **Sender shows as `0`:** expected for any RPC that reached you via the server relay — see Known limitations. Only RPCs the server receives directly carry the real sender id.
- **Scored twice / score wrong:** only the server may mutate `_score` (gated by `net_is_server()`); clients only display the broadcast `score_update`.

---

## Known limitations

Read this before judging results — several behaviours below are working-as-built, not bugs in your setup. Implemented today: UDP/netcode transport, the dedicated/headless server, host mode, the protocol + tick sync, the RPC core, server-authoritative join/leave, and basic `Transform` replication + interpolation. The rest is stub-only or unbuilt.

- **Relayed RPC sender id is lost.** Client A -> server -> client B fan-out works, but the relayed `GameEvent` arrives with `from = 0` (the server maps its own / raw peer ids to `0`). Only RPCs the **server itself** receives directly carry the true sender id. If you need the originator's id on other clients, put it in the RPC **payload** (as `net_lobby` does with `args.who`).
- **Transform replication is data-only.** `Networked` replicates `Transform` onto a **fresh client-side interpolated entity** with **no `Mesh`** (so it is invisible) and there is **no entity-ID mapping** onto pre-existing scene entities. Nothing reacts to `Added<NetworkPlayer>`, so no avatar/prefab/mesh spawns on join (Phase 4). That is why scenario #10 uses an already-placed cube and still won't show visible motion.
- **`NetworkTransform.sync_rotation` / `sync_scale` are inert.** Only `interpolate` is read; `Transform` always replicates wholesale.
- **Network Monitor shows zeros.** `NetworkStatus.rtt_ms` / `jitter_ms` / `packet_loss` / `client_id` and `ConnectedClient.rtt_ms` are never populated from Lightyear, so the panel shows static `0`s and never displays a Client ID.
- **Insecure handshake.** `Authentication::Manual` with a zero `private_key` and `protocol_id = 0` — **LAN / dev only**, no production security.
- **UDP only.** `TransportKind` (`udp`/`webtransport`/`websocket`) is parsed from `project.toml` and shown in the editor, but **no code selects a transport** — UDP is hardcoded and Lightyear is compiled with only the `udp` + `netcode` features. On **WASM, `renzora_network` is a no-op stub**, so there is no browser multiplayer.
- **Network stubs.** `net_send` / `net_send_message` / `net_spawn` `action`s are registered but are TODO stubs that never hit the wire; `net_host_server` just logs "run with --server".
- **No client input / prediction.** `PlayerInput` is defined but unregistered, and `prediction.rs` is inert (`smooth_correction` does nothing, `SNAP_THRESHOLD` unused) — client-input replication and prediction/rollback (Phases 5/6) are not built.
- **Editor network panels are read-only.** `network_monitor` / `network_entities` / `network_settings` exist, but settings is "edit `[network]` in `project.toml`" — there are no connect/host buttons yet.
- **Not built at all:** interest management, authority transfer, networked physics, reconnect, and lockstep/session layers. See `renzora_network_plan.md` for the phase roadmap.

> Don't confuse these with the editor-only dev servers: `mcp_server_plugin` (JSON-RPC MCP on port 3000) and `websocket_plugin` (dev WebSocket on port 8080) are editor tooling and do **not** touch Lightyear or game multiplayer.
