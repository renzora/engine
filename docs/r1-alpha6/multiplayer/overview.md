# Multiplayer Overview

Renzora's networking is built on Lightyear 0.26, runs over native UDP, and lives entirely in the `renzora_network` crate.

> ⚠️ **Alpha status — read this first.** Multiplayer is early and intentionally minimal. Only **native UDP** works, the handshake is **insecure** (a fixed all-zero key — LAN/dev only), and large parts are stubs (no client prediction, no avatar spawning, no alternate transports). It is enough to stand up a dedicated server, replicate `Transform`, and send RPCs between scripts. Do not ship a public, internet-facing game on it yet.

## Architecture

There is **one engine binary** (`renzora`). It is not a separate "client" and "server" — the same executable picks a network role from a launch flag:

| Launch | Role | Rendering |
|---|---|---|
| *(plain windowed launch)* | Client | full window |
| `--server` | Headless **dedicated server** | none (no GPU, no window) |
| `--host` | Windowed **listen server** (client + server in one process) | full window |

- `--host` **wins over** `--server` if both are passed.
- A `--server` or `--host` launch is **never** an editor session, even if the editor bundle is present beside the exe.
- `--server` swaps in a headless schedule that ticks at the network tick rate; there is **no separate dedicated-server executable** — it is the same binary with `--server`.
- In host mode the engine spawns the server and then a local client, which Lightyear promotes to an in-process `HostClient` (the local player never goes over UDP).

A plain client does **not** take a connect flag. It launches disconnected and connects later from a script (see [Connecting](#connecting-from-a-script) below).

> See [Server Setup](server-setup) for running and configuring the dedicated/listen server.

## Transport

**Only native UDP is implemented and compiled in.** `renzora_network` pulls Lightyear with just its `udp` and `netcode` features, and the transport is hardcoded to UDP.

```toml
[network]
transport = "udp"        # parsed and shown in the editor, but NOT yet honored
```

`TransportKind` has `udp`, `webtransport`, and `websocket` variants, and the value is read from `project.toml` and displayed in the editor's network settings panel — but **no code selects a transport from it**. `webtransport` and `websocket` are not wired up and the corresponding Lightyear features are not even enabled.

> On **WebGPU / WASM** the entire `renzora_network` crate compiles to a **no-op stub**. There is no browser multiplayer.

## Configuration

Network settings live in your project's `project.toml`:

```toml
[network]
server_addr = "127.0.0.1"
port        = 7636
transport   = "udp"
tick_rate   = 64
max_clients = 32
```

| Key | Default | Meaning |
|---|---|---|
| `server_addr` | `"127.0.0.1"` | Address the server listens on |
| `port` | `7636` | UDP port |
| `transport` | `"udp"` | Transport kind (only `udp` works today) |
| `tick_rate` | `64` | Server simulation rate in Hz |
| `max_clients` | `32` | Maximum simultaneous clients |

Server/host launches can override these from the command line; CLI flags take priority over `project.toml`:

```bash
# Headless dedicated server on a custom port and tick rate
renzora --server --port 7777 --tick-rate 30 --max-clients 16

# Windowed listen server (you play and host at once)
renzora --host
```

Recognized flags: `--server`, `--host`, `--port`, `--addr` / `--address`, `--tick-rate`, `--max-clients`. (These are parsed only for server/host launches.)

> When building from source, the cargo aliases are the quickest way to launch a role: `cargo server` runs the dedicated server, `cargo runtime` runs a plain game client.

## State replication

Replication is component-based and driven by a marker:

- Add the **`Networked`** marker to an entity and the server automatically inserts Lightyear's `Replicate::to_clients(All)` (plus an interpolation target on remotes).
- **`Transform`** is the one gameplay component that actually replicates, with snapshot interpolation (linear `lerp`) on the receiving side so remote entities move smoothly between updates.
- The fixed protocol replicates exactly: `Networked`, `NetworkId`, `Name`, `NetworkPlayer`, `NetworkOwner`, and `Transform`. There is **no generic component replication and no mesh replication**.

> ⚠️ Replication today creates a **new** entity on each client carrying the interpolated `Transform`; it does **not** map network IDs onto your pre-existing scene entities, and the spawned entity has no mesh, so it is invisible until you give it one. `NetworkTransform.sync_rotation` / `sync_scale` are currently **inert** — only `interpolate` is read, and `Transform` always replicates in full. Client-side **prediction and reconciliation are not implemented** (the prediction module is an inert stub).

See [State Replication](replication) for the details and current limitations.

## Scripting API

Networking is exposed to **Lua only**. Rhai has no networking functions (it is a strict subset and omits `rpc`/`net_*`), so write multiplayer logic in `.lua` scripts.

### Status globals

| Function | Returns |
|---|---|
| `net_is_server()` | `true` on the dedicated/host server |
| `net_is_client()` | `true` when networking is active and this is not the server |
| `net_is_connected()` | connected to a server (client) or running (server) |
| `net_player_count()` | connected client count (server only; `0` elsewhere) |

```lua
function on_update()
    local role = net_is_server() and "SERVER" or "CLIENT"
    local conn = net_is_connected() and "connected" or "offline"
    action("ui_set_text", { name = "NetStatus", text = role .. " - " .. conn })

    -- Player count is authoritative on the server/host only.
    if net_is_server() then
        action("ui_set_text", { name = "Players",
            text = "Players: " .. tostring(net_player_count()) })
    end
end
```

### RPCs

`rpc(name, args)` broadcasts a remote procedure call. `args` is an arbitrary table — numbers, strings, bools and `{x, y, z}` vectors round-trip. It is sent reliably; a client sends to the server, and the server relays to **every other** client (never echoing back to the sender).

```lua
function on_update()
    if is_key_just_pressed("KeyP") then
        rpc("ping", { msg = "hello", at = elapsed })   -- to every other peer
        print_log("sent ping")
    end
end

-- Fires on remote peers when an RPC arrives. A script with no on_rpc ignores RPCs.
function on_rpc(name, args, from)
    if name == "ping" then
        print_log("got ping from " .. tostring(from) .. ": " .. tostring(args.msg))
    end
end
```

> ⚠️ **The original sender's id is lost through the server relay.** Only the server sees the true peer id; on a relayed client, `from` is always `0`. Use a field inside `args` if you need to identify who sent an RPC. `rpc()` is also a no-op (with a warning) when you are not connected.

### Lifecycle hooks (server-authoritative)

These hooks fire **only on the server/host**, so player presence is authoritative. Scripts also run on the headless dedicated server, which is where you put authoritative game logic.

| Hook | When it fires |
|---|---|
| `on_rpc(name, args, from)` | a networked RPC arrives |
| `on_player_joined(id)` | a peer connects (server/host only) |
| `on_player_left(id)` | a peer disconnects (server/host only) |

```lua
local count = 0

-- These run only on the server/host.
function on_player_joined(id)
    count = count + 1
    print_log("player " .. tostring(id) .. " joined — " .. count .. " online")
    rpc("lobby", { count = count, who = id, joined = true })   -- tell the clients
end

function on_player_left(id)
    if count > 0 then count = count - 1 end
    rpc("lobby", { count = count, who = id, joined = false })
end

-- Runs on every client to receive the broadcast presence update.
function on_rpc(name, args, from)
    if name == "lobby" then
        action("ui_set_text", { name = "Players", text = "Players: " .. tostring(args.count) })
    end
end
```

### Connecting from a script

A plain client launches disconnected. Connection is driven through the generic `action()` verb, not a bare function or a CLI flag:

```lua
function on_ready()
    action("net_connect", { address = "127.0.0.1", port = 7636 })
end

-- Later, to disconnect:
-- action("net_disconnect")
```

Attach a single connector script to one entity; your other networking scripts assume the connection already exists. The matching test scripts shipped in `assets/scripts/` are a good starting point: `net_connect`, `net_hud`, `net_lobby`, `net_chat`, `net_score`, `multiplayer_ping`, and `net_move`.

> ⚠️ The following script verbs exist but **do nothing on the wire yet** (they log a TODO): `action("net_send", ...)`, `action("net_send_message", ...)`, `action("net_spawn", ...)`, and `action("net_host_server", ...)` (the last just tells you to launch with `--server`). Use `rpc()` for messaging today.

## Editor panels

`renzora_network_editor` adds inspector cards for `Networked` and `NetworkTransform` plus three native panels: **Network Monitor**, **Network Entities**, and **Network Settings**.

> The Settings panel is **read-only** — edit `[network]` in `project.toml` (there are no connect/host buttons yet). The Monitor panel's RTT, jitter, packet-loss, and Client ID rows are **not populated from Lightyear** and currently always show zeros.

## Not implemented yet

To set expectations, these are defined but **stub-only or absent** in this alpha:

- Client input replication (`PlayerInput` exists but is never registered).
- Client-side prediction and server reconciliation (the prediction module is inert).
- Automatic avatar/prefab spawning on join (nothing reacts to a new `NetworkPlayer`).
- Alternate transports (WebTransport / WebSocket), interest management, authority transfer, networked physics, and any form of security.

## Related (not game multiplayer)

These run their own servers and are unrelated to Lightyear — don't confuse them with multiplayer:

- **`renzora_auth`** — editor-only sign-in/register against the renzora.com API.
- **`mcp_server_plugin`** — editor MCP server (JSON-RPC) for tooling.
- **`websocket_plugin`** — editor dev WebSocket server for remote editor commands.
