# Networking Test Plan

A hands-on, in-engine checklist to verify the multiplayer that's built today —
the popular scenarios (connect, presence, chat, authoritative score, RPC relay,
disconnect, host mode, transform sync), all driven by scripts + an on-screen
HUD. Everything here uses **only shipped primitives**; nothing is hardcoded.

> Scripts live in `assets/scripts/`: `net_connect`, `net_hud`, `net_lobby`,
> `net_chat`, `net_score`, `multiplayer_ping`, `net_move`.

---

## 1. Build the runtime
```bash
renzora build              # → dist/<platform>/renzora(.exe)
```
The runtime is your client/server (one binary, mode chosen by flag).

## 2. Build the HUD (one-time, in the editor)
Add a **UI Canvas** named `HUD`, and inside it five **Text** widgets named
exactly:

| Widget name | Shows |
|---|---|
| `NetStatus` | role + connection (SERVER/CLIENT — connected/offline) |
| `Players`   | authoritative player count |
| `Events`    | last join/leave notice |
| `Chat`      | last chat line |
| `Score`     | server-authoritative score |

Scripts find widgets **by Name**, so the names must match. (If a widget is
missing, the matching `ui_set_text` is a harmless no-op — every script also
logs to the console, so you can test HUD-free too.)

## 3. Attach the scripts
Put these on entities in your scene (an empty named "Net" is fine — several can
share one entity):

- `net_connect.lua` — **exactly one** per client; set `address`/`port`.
- `net_hud.lua`, `net_lobby.lua`, `net_chat.lua`, `net_score.lua`,
  `multiplayer_ping.lua` — attach the ones you want to exercise.
- `net_move.lua` — on a cube that's in the scene on every peer; add the
  `Networked` component to that cube in the inspector.

## 4. Launch
**Option A — dedicated server + 2 clients (most realistic):**
```bash
./dist/windows-x64/renzora --server   # console 1: headless server, port 7636
./dist/windows-x64/renzora            # window 2: client A
./dist/windows-x64/renzora            # window 3: client B
```
**Option B — host + 1 client (quick):**
```bash
./dist/windows-x64/renzora --host     # window 1: you're server + a player
./dist/windows-x64/renzora            # window 2: another player
```
Always start the server/host **first**. Default port **7636** (`--port N`).

> Paths are platform-specific: the binary is `renzora.exe` on Windows and a
> bare `renzora` on Linux/macOS (under `dist/<platform>/`). The
> `renzora-runtime` name only applies to **exported/distributed** templates,
> not the locally-built binary you launch here.

---

## 5. Scenario checklist

| # | Scenario | Steps | Expected ✅ |
|---|---|---|---|
| 1 | **Connect** | Start server, then a client (with `net_connect` + `net_hud`) | Client HUD `NetStatus` → `CLIENT — connected`; server → `SERVER — connected` |
| 2 | **Presence / join** | With `net_lobby` attached, connect a 2nd client | Server console: `player <id> joined — N online`; all HUDs `Players: N` + `Events: player <id> joined` |
| 3 | **Player count** | Connect/disconnect clients | `Players` rises/falls and matches the number of connected clients |
| 4 | **RPC broadcast + no self-echo** | `multiplayer_ping` on all; press **P** on client A | Client B + server log `got ping from <A>`; **A does not** log its own ping |
| 5 | **RPC args + sender id** | `net_chat`; press **1/2/3** on a client | Other peers' `Chat` → `[player <id>] <text>`; sender sees `you: <text>` |
| 6 | **Server-authoritative score** | `net_score`; press **K** on a client | Only the server tallies; **all** `Score` labels jump to the same N; server logs who scored |
| 7 | **Server relay (client→client)** | Two clients + ping/chat; act on client A | Client **B** receives it (proves the server relays peer→peer, not just peer→server) |
| 8 | **Disconnect** | Close one client window | Server: `player <id> left — N online`; remaining HUDs `Players` drops, `Events: player <id> left` |
| 9 | **Host mode** | Launch with `--host`, connect one client | Host HUD `SERVER — connected` and it counts as a player; score/chat work both ways |
| 10 | **Transform replication** | `net_move` on a shared scene cube w/ `Networked` | On clients, the cube tracks the server's circular motion (smoothly interpolated) |

---

## 6. Troubleshooting
- **Client won't connect:** start the server first; check the port matches
  (`--port` / `net_connect` `port` prop, default 7636); same machine uses
  `127.0.0.1`, LAN uses the server's IP; check firewall for UDP 7636.
- **HUD not updating:** widget `Name` must match the script string exactly
  (case-sensitive). Watch the console — scripts log there too.
- **`on_player_joined` never fires:** it's **server-only**. It runs on the
  `--server`/`--host` process, not on plain clients (clients get the broadcast
  via `net_lobby`'s `on_rpc`).
- **Scored twice / score wrong:** make sure only the server mutates `_score`
  (it's gated by `net_is_server()`); clients only display `score_update`.

## 7. Not covered yet (future phases)
These don't work *yet* — see `renzora_network_plan.md`:
- **Server-spawned entity visuals** — a cube spawned only on the server doesn't
  get its mesh on clients (Phase 4 prefab-spawn replication). That's why
  scenario #10 uses a cube already present in the scene on every peer.
- Client input → server, client-side prediction/rollback (Phases 5–6).
- WebTransport/WebSocket (browser clients), Steam, secure netcode, reconnect.
