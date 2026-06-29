# Server Setup

Run a dedicated or listen server using the same `renzora` binary, selected entirely by command-line flags at startup.

## How servers work

Renzora multiplayer is built on [Lightyear](https://github.com/cBournhonesque/lightyear) 0.26 (`crates/renzora_network`). There is **no separate server executable and no "server export"** — the dedicated server is the *same* `renzora` binary you ship as the game, launched with a flag. Whether a process is an editor, a game client, or a server is decided at runtime in `src/main.rs`, not at compile time.

- **Transport is native UDP only.** Lightyear is compiled with just the `udp` and `netcode` features. The `transport` field in config (`udp`/`webtransport`/`websocket`) is parsed and shown in the editor, but **no other transport is wired up** — UDP is hardcoded.
- **Networking is native-only.** On WebAssembly the entire `renzora_network` crate compiles to a no-op stub, so web builds cannot host or join a server.
- **The handshake is insecure.** The server authenticates clients with a fixed zero key (`private_key = [0u8; 32]`, `protocol_id = 0`). This is fine for LAN and local development; do **not** expose it to the open internet as-is.

> Server-side scripts *do* run on the headless dedicated server (the script runtime is active in standalone mode), so server-authoritative gameplay written in Lua executes server-side.

## Run modes

All three modes are the same binary. The mode is chosen by flags (`--host` wins if both `--host` and `--server` are passed). A server or host launch is **never** an editor session, even if the editor bundle is sitting beside the exe.

| Launch | Mode | Rendering | Network role |
|---|---|---|---|
| `renzora` (no flag) | Editor (if `renzora_editor` bundle is present) or game | Windowed | Client; connects on demand |
| `renzora --server` | Headless **dedicated server** | None (no GPU, no window) | Server only |
| `renzora --host` | Windowed **listen server** | Windowed | Server **and** local client in one process |

- **`--server`** inserts a `DedicatedServer` marker and boots headless: wgpu backends disabled, no window, `WinitPlugin` off, driven by a fixed-rate runner at the network tick rate, plus `NetworkServerPlugin`.
- **`--host`** inserts a `HostServer` marker, keeps full rendering, and runs the client half alongside the server. Lightyear promotes the local player to an in-process `HostClient`, so the host player never goes through UDP.

> The exported game binary takes the same flags. If your shipped build is named `mygame`, then `mygame --server` and `mygame --host` behave identically to the table above.

## Starting a server

Run the dedicated (headless) server:

```bash
# Default config (port 7636, tick rate 64, max 32 clients)
renzora --server

# Override any of the defaults on the command line
renzora --server --addr 0.0.0.0 --port 7636 --tick-rate 64 --max-clients 32
```

Run a windowed listen server (host plays in the same process):

```bash
renzora --host --port 7636
```

During engine development you can use the renzora CLI, which builds (inside the Docker toolchain) and runs the dedicated server:

```bash
renzora run -- --server
```

## Configuration

Settings come from two places, in order of precedence:

1. **Command-line flags** (highest priority).
2. The **`[network]` section of `project.toml`** for anything a flag did not set.
3. Built-in defaults if neither is present.

### Command-line flags

| Flag | Default | Meaning |
|---|---|---|
| `--port <u16>` | `7636` | Port to listen on |
| `--addr <ip>` / `--address <ip>` | `127.0.0.1` | Address to bind / advertise |
| `--tick-rate <u16>` | `64` | Server simulation rate (Hz) — also drives the headless run loop |
| `--max-clients <u16>` | `32` | Maximum connected clients |

> There are no `--password`, `--name`, anti-cheat, or packet-size flags — those do not exist in the engine. The four flags above are the complete set the server reads.

### project.toml

Put network defaults in your project's `project.toml`. Any field a flag does not override is taken from here:

```toml
[network]
server_addr = "127.0.0.1"   # bind / connect address
port        = 7636
transport   = "udp"          # only "udp" is functional today
tick_rate   = 64             # Hz
max_clients = 32
```

> The `[network]` section is optional. If it is absent the engine uses the built-in defaults (loopback, port 7636, 64 Hz, 32 clients, UDP).

## Connecting clients

A plain `renzora` launch is a windowed **client**. It does not connect to anything automatically — connection is requested dynamically from a script via the `action()` event bus, which sets a `PendingNetworkConnect` resource that the network plugin acts on:

```lua
-- net_connect.lua — attach to ONE entity (e.g. an empty named "Net").
-- Start the server first:  renzora --server
function props()
    return {
        address = { value = "127.0.0.1", hint = "Server address" },
        port    = { value = 7636,        hint = "Server port (matches --port / project.toml)" },
    }
end

function on_ready()
    action("net_connect", { address = address, port = port })
    print_log("connecting to " .. address .. ":" .. tostring(port))
end
```

To disconnect, call `action("net_disconnect")`.

> **Editor note:** the editor's three network panels — **Network Monitor**, **Network Entities**, and **Network Settings** — are present but the Settings panel is **read-only** ("Edit `[network]` in project.toml"); there are **no connect/host buttons in the editor yet**, and the Monitor's RTT / jitter / packet-loss / Client ID rows are not populated (they show static zeros). Drive connections from a `net_connect` script and launch servers/hosts from the command line.

## Server-side scripting

Networking is exposed only to **Lua** scripts (native platforms). The Rhai backend has no networking surface at all. The full list of bare globals:

| Function | Returns / effect |
|---|---|
| `net_is_server()` | `true` on a dedicated server or host |
| `net_is_client()` | `true` on a connected client |
| `net_is_connected()` | `true` once a connection is established |
| `net_player_count()` | Number of connected players |
| `rpc(name, args)` | Send a remote procedure call (reliable channel) |

And the network lifecycle hooks (Lua only — they fire on the **server/host**):

| Hook | When |
|---|---|
| `on_rpc(name, args, from)` | An RPC arrived |
| `on_player_joined(id)` | A client connected |
| `on_player_left(id)` | A client disconnected |

> Player join/leave are **server-authoritative**: `on_player_joined` / `on_player_left` fire only on the server or host. There is **no** `on_server_tick`, `on_player_connect`, or `on_player_disconnect` hook — use `on_update` (which also runs on the headless server) for per-tick logic, and the hooks above for connection events.

### RPCs and the relay model

`rpc(name, args)` always uses the **reliable** channel. A client's RPC goes to the server; the server relays it to every *other* client (the sender never receives its own echo). This makes a clean client-asks / server-decides pattern:

```lua
-- net_score.lua — server-authoritative score using rpc() + net_is_server().
function props()
    return { add_key = { value = "KeyK", hint = "Press to request a point" } }
end

local total = 0

function on_update()
    if is_key_just_pressed(add_key) then
        rpc("score_request", {})         -- ask the server (broadcast; only it acts)
    end
end

function on_rpc(name, args, from)
    if name == "score_request" then
        if net_is_server() then           -- only the server tallies
            total = total + 1
            rpc("score_update", { total = total })   -- tell everyone
            action("ui_set_text", { name = "Score", text = "Score: " .. tostring(total) })
        end
    elseif name == "score_update" then
        action("ui_set_text", { name = "Score", text = "Score: " .. tostring(args.total) })
    end
end
```

> **Caveat — the sender id is lost on relay.** When an RPC is relayed through the server to other clients, it arrives with `from = 0`. Only the server itself sees the true peer id of the originating client; relayed clients always see `0`. Don't rely on `from` for client-to-client identity.

### Stubs to avoid

These network actions are registered but are **not implemented** (they only log and never touch the wire). Do not build on them yet:

- `action("net_send", ...)` / `action("net_send_message", ...)`
- `action("net_spawn", ...)`
- `action("net_host_server", ...)` — only logs "run with `--server`"

## What the server does today

Implemented and working:

- The dedicated (`--server`) and host (`--host`) modes, tick synchronization, and the protocol handshake.
- `rpc()` send/receive/relay and the join/leave lifecycle hooks.
- Basic `Transform` replication with snapshot interpolation. Adding the `Networked` marker to an entity auto-inserts Lightyear's `Replicate` (to all clients) plus an interpolation target. See [State Replication](/docs/r1-alpha5/multiplayer/replication) for details.

Not yet implemented (do not assume these work):

- Client input replication, prediction, and rollback.
- Automatic avatar/prefab spawning on join (nothing reacts to a new `NetworkPlayer` yet).
- Interest management, authority transfer, networked physics.
- Alternate transports (WebTransport / WebSocket) — UDP only.
- Any real security on the handshake — **LAN / dev only**.

> Several engine systems sound network-related but are **not** game multiplayer and do not use Lightyear: `renzora_auth` (editor-only renzora.com sign-in), the editor's MCP server (`mcp_server_plugin`, JSON-RPC on port 3000), and the editor's dev WebSocket server (`websocket_plugin`, port 8080). None of these are part of your game server.

## Related

- [Multiplayer Overview](/docs/r1-alpha5/multiplayer/overview) — the big picture
- [State Replication](/docs/r1-alpha5/multiplayer/replication) — the `Networked` marker and `Transform` sync
- [Lua](/docs/r1-alpha5/scripting/lua) — the full scripting surface (networking is Lua-only)
- [Exporting](/docs/r1-alpha5/exporting/overview) — building the binary you run with `--server`
