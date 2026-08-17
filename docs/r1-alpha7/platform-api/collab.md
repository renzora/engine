# Collaboration Relay API

Endpoints backing [Collaborating on a Scene](../editor/collaboration.md). A
session hosted through renzora.com works between any two people, without either
of them being reachable from the internet — both editors connect *outward* and
the server forwards bytes between them.

All endpoints require a signed-in account (`Authorization: Bearer <access
token>`), except the relay socket itself, which takes the token as a query
parameter because a WebSocket handshake cannot carry headers.

## The room lifecycle

```
POST /api/collab/sessions        → host gets a code
GET  /api/ws/collab/:code        → host connects, session goes live
GET  /api/ws/collab/:code        → guests connect
```

A room exists only while its host is connected, and is held in memory rather
than in the database — the room *is* the connection. A server restart therefore
ends every session, which is what dropping the sockets does anyway.

Rooms are swept when the host never connects within **5 minutes**, and after a
**12-hour** maximum session length. An account may host **3** concurrent
sessions.

## `POST /api/collab/sessions`

Create a room.

```json
{ "project": "mygame" }
```

```json
{
  "code": "K7MPQ2XZ",
  "ws_url": "wss://renzora.com/api/ws/collab/K7MPQ2XZ",
  "expires_in_secs": 43200
}
```

`ws_url` is returned rather than assembled by the client so that staging and
local deployments answer with their own address.

The code's alphabet excludes `0/O` and `1/I/L`, because it gets read aloud and
typed by hand.

## `GET /api/collab/sessions/:code`

What a joiner sees before committing.

```json
{
  "code": "K7MPQ2XZ",
  "host_username": "ada",
  "project": "mygame",
  "guests": [{ "peer": 1, "username": "grace" }],
  "host_online": true,
  "ws_url": "wss://renzora.com/api/ws/collab/K7MPQ2XZ"
}
```

Readable by any signed-in user holding the code. **The code is the capability** —
there is no separate allowlist. That is the same model as a meeting link, and it
is what lets a host invite someone who is not on their friends list. Codes are
~41 bits of entropy, a wrong one 404s, and there is nothing to enumerate.

## `DELETE /api/collab/sessions/:code`

Host only. Ends the session and disconnects everyone.

## `POST /api/collab/sessions/:code/invite`

Host only, and **friends only** — without that restriction this is a way to push
a notification to any account by id.

```json
{ "user_id": "8f14e45f-…" }
```

Delivers both a notification row and a `collab_invite` live event, so an editor
that is already running can offer a one-click join instead of making the user
read a code out of a notification and type it back in:

```json
{
  "event": "collab_invite",
  "data": {
    "code": "K7MPQ2XZ",
    "host_username": "ada",
    "project": "mygame",
    "ws_url": "wss://renzora.com/api/ws/collab/K7MPQ2XZ"
  }
}
```

The notification is the *delivery*, not the permission: anyone with the code can
join whether they were invited or not.

## `GET /api/ws/collab/:code?token=…`

The relay socket. Role is decided by the server: the first connection from the
account that created the room becomes the host, and **every** later connection
is a guest — including a second one from that same account, so testing with two
editors signed in as yourself works. That cannot strand a live host, because a
host disconnecting takes the room with it.

| Status | Meaning |
|---|---|
| 401 | Bad or expired token |
| 404 | No such session |

### Clients must send keep-alives

**Ping at least every 60 seconds.** A session is idle whenever nobody is
editing, and Cloudflare closes an idle WebSocket after about 100 seconds. A
client that stays silent gets its session torn down on its own, several minutes
in, reported as `Connection reset without closing handshake` — with nothing the
user did to cause it. The editor pings every 30 seconds.

### The envelope

The relay does not parse the editor's protocol — payloads are opaque, and its
whole job is deciding which socket each one goes to. That is deliberate: the
editor's protocol changes with the editor, and a relay that understood it would
have to be redeployed in lockstep with a desktop app people upgrade whenever they
feel like it.

A direct session gives the host one socket per guest, so "send to guest 3" is
just "write to socket 3". Through a relay the host has one socket carrying
everyone, so every **binary** message is `[peer: u32 LE][payload…]`:

| Direction | `peer` means |
|---|---|
| host → relay | send to this guest; `0xFFFFFFFF` means all of them |
| relay → host | this guest sent it |
| guest → relay | **ignored** — rewritten server-side to the guest's own id |
| relay → guest | always `0` (the host) |

A guest's envelope is rewritten rather than trusted. Otherwise one guest could
forge traffic as another, and the host would have no way to tell them apart.

Payloads must be at least 4 bytes and at most **32 MB**.

### The byte layout, exactly

This is the canonical statement of the header. The engine and the server are
separate codebases in separate repositories, deployed independently, and each
implements this on its own — so both assert these **same literal bytes** in their
tests rather than trusting a shared description:

| What | Bytes |
|---|---|
| Peer 3, payload `hi` | `03 00 00 00 68 69` |
| The host (peer 0), payload `hi` | `00 00 00 00 68 69` |
| Broadcast, payload `hi` | `FF FF FF FF 68 69` |
| Peer 1, empty payload | `01 00 00 00` |

Little-endian, four bytes, payload immediately after. An empty payload is valid.

The reason this is written down to the byte rather than left as "a u32": getting
the endianness wrong does not fail loudly. A message for peer 3 read big-endian
addresses peer `50331648`, which matches nobody, so it is dropped — and the only
symptom is a collaborator whose edits silently never arrive.

Where the tests live:

- Server — `crates/api/src/collab.rs`, `mod tests` (website repo). Also drives
  two real WebSocket clients through a real relay to check routing, guest-tag
  rewriting, and that guests never see each other.
- Engine — `crates/renzora_collab/src/relay.rs`, `mod tests`.

Both use the same constant names (`PEER_3_HI`, `HOST_HI`, `BROADCAST_HI`), so a
change on one side that is not mirrored is easy to spot.

### Control frames

Relay control travels as WebSocket **text** frames carrying JSON, so it can never
be confused with payload — and so the relay needs no message type of its own
inside the editor's protocol.

| Event | To | Meaning |
|---|---|---|
| `ready` | both | Carries `role` (`host`/`guest`) and your `peer` id |
| `peer_joined` | host | `peer`, `username`, `user_id` |
| `peer_left` | host | `peer` |
| `host_gone` | guests | The session ended; `reason` says why |

`peer_joined` and `peer_left` exist because a relayed host has no per-guest
socket whose open and close could announce it.

### Backpressure

Each connection has a bounded send queue (512 messages). A full queue closes
*that* connection rather than dropping messages from it: the relay cannot tell a
discardable camera update from a scene snapshot, and silently losing the latter
desynchronises two people's projects with no error anywhere. Cutting a peer that
cannot keep up at least fails loudly, on the side that is failing.
