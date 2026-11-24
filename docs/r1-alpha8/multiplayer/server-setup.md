# Server Setup

A server is the same `renzora` binary you ship as the game, started with a flag. There is no separate server executable.

## Run modes

| Launch | Mode | Window | Player of its own |
|---|---|---|---|
| `renzora --server` | Dedicated server | none (headless, no GPU) | no |
| `renzora --host` | Listen server | yes | yes, peer id `1` |
| `renzora --connect host:port` | Client, joining at startup | yes | yes |

`--host` wins if both `--host` and `--server` are passed. A server or host launch is never an editor session. An exported game named `mygame` takes the same flags: `mygame --server`.

The dedicated server runs the scene and every script, headless, at the tick rate. That is where server-authoritative game rules run: guard them with `renzora::network::is_server(world)`.

## Flags

| Flag | Default | Meaning |
|---|---|---|
| `--port <u16>` | `7636` | UDP port to listen on |
| `--addr <ip>` / `--address <ip>` | `127.0.0.1` | The address clients use (the server listens on every interface) |
| `--tick-rate <u16>` | `64` | Simulation rate of the headless server, in Hz |
| `--max-clients <u16>` | `32` | Slots; a client past this is told the server is full |
| `--connect [addr[:port]]` | | Client only: join at startup. Bare `--connect` uses `[network]` |

A flag wins over `project.toml`.

## project.toml

```toml
[network]
server_addr = "127.0.0.1"  # where clients connect by default
port        = 7636
tick_rate   = 64
max_clients = 32
play_mode    = "host"      # editor Play: "single", "host" or "server"
play_clients = 1           # editor Play: extra client windows
```

The section is optional. A server reads it from the game's own project wherever it lives (the rpak, `--project`, or `./project.toml`), once at startup. A client uses `server_addr` and `port` when a connect call leaves them out.

`transport` is accepted for older projects and ignored: UDP is the only transport.

## What a server does

- **Accepts players.** The handshake checks the client is the same game (the project name and wire version must match), proves the client receives at the address it claims, and assigns it a random peer id. A full server or a different game is refused with a reason the client shows in `NetworkStatus::last_error`.
- **Replicates** every `Networked` entity. See [State Replication](replication).
- **Routes RPCs**, stamping each with the real sender's id.
- **Spawns avatars** from the scene's `NetworkPlayerSpawner`, one per player (the host's included), and despawns everything a player owned when they leave.
- **Times out** a client silent for 8 seconds, and tells clients when it shuts down so they do not wait for that.

## Watching a session

`renzora::network::NetworkStatus` has everything: role, state, peer id, round trip, jitter, packet loss, bytes in and out per second, the player list, and how many entities are replicated. A dedicated server logs joins and leaves:

```
[network] Dedicated server listening on UDP 0.0.0.0:7636 (max 32 players, 64 Hz)
[network] Player 4825510034291 joined
[network] Spawned avatar for player 4825510034291
```

Raise its detail with `RUST_LOG=info,renzora_network=debug`.

## Security

The handshake stops a stranger from taking slots with forged addresses, and from joining with a different build. Nothing is encrypted and the server takes an owner's word for where its avatar is. Fine on a LAN or among friends; not yet for an open internet game with something worth cheating for.

## Related

- [Multiplayer Overview](overview)
- [State Replication](replication)
- [Exporting](/docs/r1-alpha8/exporting/overview): the binary you run with `--server`. The **Multiplayer (networking)** export capability must be on.
