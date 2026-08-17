# Collaborative Editing — How It Works

`crates/renzora_collab` (Editor scope). This page is the architecture; the
user-facing guide is [Collaborating on a Scene](../editor/collaboration.md).

## The three decisions

**The host is the authority.** One machine owns the document; everyone else's
edits are proposals it applies and relays. Not because peer-to-peer merging is
impossible, but because the alternative is a distributed consensus problem
underneath a level editor. It also answers "whose version gets saved" honestly:
the host's, because the host is the one with the project open.

**State is replicated, not operations.** The obvious design is to replicate edits
— "move entity 4 to (1,2,3)" — and replay them. That was rejected because the
editor has on the order of a hundred distinct mutation paths (the gizmo, every
inspector field, the hierarchy, terrain, importers, scripts), and operation
replication means finding every one of them, giving it a serializable form, and
keeping it in sync forever. Miss one and it silently does not replicate.

Replicating state inverts that. Nothing announces an edit; the sync notices that
an entity *changed*, whoever changed it and however. A tool written next year
replicates with no knowledge that collaboration exists. The price is that a
change is described by its result rather than its intent, so simultaneous edits
to one entity are last-writer-wins — which is what leases are for.

**The transport is replaceable, and both now exist.** Everything above `link` is
written in messages rather than sockets, which is what let the relay land as a
new module rather than a rewrite — `session` cannot tell a relayed guest from a
direct one.

## Module map

| Module | Job |
|---|---|
| `protocol` | The `CollabMsg` vocabulary and its length-prefixed framing |
| `link` | One duplex connection, abstracted away from its transport |
| `tcp` | Direct transport: a listener and a connector |
| `relay` | Relayed transport: one WebSocket to renzora.com, multiplexed |
| `online` | Creating and finding relay rooms over the site's REST API |
| `session` | Peers, roles, handshake, the link pump |
| `identity` | `CollabId` — names for entities that mean the same on both machines |
| `sync` | Change detection, snapshotting, applying |
| `files` | Manifest + chunked transfer of the project itself |
| `lease` | Claiming entities so two people don't fight over one |
| `presence` | Cameras and selections |
| `panel` | The Collaborate panel |

## Identity

`CollabId(u64)` is a component that derives **neither `Reflect` nor a type
registration**. The scene serializer only extracts registered components, so
these ids are invisible to it — they can never leak into a `.scene` file, never
appear in a diff, and never survive a save/load to be mistaken for durable
identity. They travel in each message's own id table instead.

The id space is partitioned rather than host-assigned: the top 16 bits are the
peer's slot, the bottom 48 a counter. A guest that spawns an entity needs an id
for it *before* the host has seen it, or the entity cannot be described in the
message announcing it; partitioning lets every peer mint freely with no round
trip and no possibility of collision. The host is slot 0.

Ids are also **self-healing**. A `CollabId` outlives the session that minted it,
so leaving and rejoining would otherwise reuse ids from a slice of the space this
peer no longer owns. `sync::id_is_current` checks the id against the registry
rather than trusting the component, and a stale one is simply re-minted.

## Change detection

Two mechanisms, because neither alone is enough:

- **Component change ticks.** Every component records the tick it was last
  written; an entity is dirty if any component changed since it was last synced.
  Catches every mutation and every component *addition*.
- **Archetype identity.** A component *removal* leaves no tick behind — the data
  is gone, with nothing to carry a timestamp. But an entity's archetype **is**
  its component set, so a changed `ArchetypeId` means the set changed, and
  diffing the two archetypes says exactly which components went away. One
  `ArchetypeId` per entity costs four bytes and replaces a stored copy of every
  entity's component list.

The scan runs at 15 Hz, not per frame. It is exclusive-world work, and it also
coalesces a gizmo drag's sixty per-second writes into one message.

## The replication primitive

Two functions in `renzora_engine::scene_io` do the actual work, both shared with
delete-undo:

```rust
snapshot_entities(world, entities, Descend::ExactSet) -> Option<String>
apply_entity_snapshot(world, bsn, seed, ExternalParents::MappedOnly) -> EntityHashMap<Entity>
```

`apply_entity_snapshot` is the important one. `write_to_world` remaps every
entity reference through the map it is handed, and an id **already present in
that map resolves to the mapped entity instead of a fresh one** — so pre-seeding
`snapshot entity → local entity` makes the write land on the entity that is
already there, keeping its identity, its children, and anything holding a
reference to it. That is the whole mechanism behind live co-editing.

Two parameters exist purely to keep the collab and undo cases apart:

- **`Descend::ExactSet`** vs `Subtree`. Undo restoring a deleted parent must
  bring its children back. Replication must *not* descend, or nudging a tilemap
  layer would re-send its thousands of tile children several times a second.
- **`ExternalParents::MappedOnly`** vs `Identity`. Undo's snapshot came from this
  same world moments ago, so a live entity with the same id *is* the parent.
  A collaborator's ids mean nothing locally, and an unmapped one that happens to
  be live is a coincidence — honouring it would reparent onto an unrelated
  object.

Applying **adds and overwrites but never removes**, which is why `EntityUpsert`
carries a separate `removed: Vec<(u64, Vec<String>)>` of type paths. Without it,
turning a light off on one machine would leave it on forever on the other.

## System order

```rust
(pump_links, apply_inbox, scan_and_send, poll_compare, claim_selection, broadcast_presence).chain()
```

Chained deliberately: the pump turns socket traffic into a queue, the apply
drains that queue into the world, and only then is it meaningful to ask what
changed. Scanning first would send a peer their own edit back a frame later.

`pump_links` is a plain system and never touches the scene — it handles
handshakes, presence and peer bookkeeping, and pushes anything needing world
access into `CollabInbox`. That split is what stops the editor taking `&mut
World` once a frame just to notice a peer's camera moved.

### Echo suppression

Applying a peer's change marks those components changed locally, so the next scan
would "notice" it and send it straight back. `apply_snapshot` stamps every
touched entity as synced at the current tick, which makes the change invisible to
the scan that follows. `Track::synced` therefore means *last brought into
agreement*, in either direction — not *last sent*.

## What counts as the document

`sync::replicable` mirrors the save filter in `scene_io::save_scene`, because it
is the same question: a session replicates the document a save would write.
Requires `Name`; excludes `HideInHierarchy`, `EditorCamera`, `Persistent` and
gamepads; excludes descendants of `MeshInstanceData` and `SceneInstance` (rebuilt
from source on the far side); and walks ancestors via `has_hidden_ancestor`,
because editor chrome tags only its root — a bare `Without<HideInHierarchy>`
would ship the other editor's dock tabs as scene content.

## The relay

Two people on ordinary home connections cannot reach each other: both are behind
NAT and neither can accept an inbound connection. Direct TCP is a LAN feature. The
relay is what makes this "invite a friend".

Both editors connect **outward** to `wss://renzora.com/api/ws/collab/:code`, which
every network allows, and the server forwards bytes between them without parsing
them. The server side is `crates/api/src/collab.rs` in the website repo; the wire
contract is documented in [Collaboration Relay API](../platform-api/collab.md).

### The `Acceptor` seam

A host needs *somewhere guests arrive from*. `link::Acceptor` is that, and both
transports produce one — `tcp::listen` from a bound port, `relay::host` from a
single socket the server multiplexes. `CollabSession` holds an `Acceptor` and
never learns which it has.

### Multiplexing

Direct hosting gives one socket per guest, so a guest arriving *is* a socket
arriving. Relayed hosting has one socket carrying everyone, so `relay` has to:

- **Tag messages.** Every binary frame is `[peer: u32 LE][frame]`. A guest's tag
  is rewritten server-side to its own id, so one guest cannot forge traffic as
  another.
- **Learn about arrivals from the server.** `peer_joined` / `peer_left` arrive as
  JSON *text* frames, because there is no per-guest socket whose open and close
  could say it. Text vs binary is also what keeps relay control from needing a
  variant inside `CollabMsg`.
- **Fan the outbound side back out.** Each guest `Link` owns a queue, and one
  socket has to drain all of them. Rather than teach `Link` about tagging — which
  would push relay concerns into the type whose whole purpose is not to have any
  — each guest gets a small forwarder thread that stamps its peer id on and
  pushes into the shared write queue. Sessions have a handful of participants, so
  a thread each is the cheap option as well as the tidy one.

### A read timeout *is* safe here

`relay` sets a 250 ms socket read timeout, which `tcp` must never do. The
difference is `read_exact`: a WebSocket library hands over whole messages, so a
timed-out read has either produced a message or produced nothing, and there is no
half-consumed frame to lose. Over a raw stream there is — see the framing section.

### Rooms are created over REST, not on the socket

`online` posts to `/api/collab/sessions` to get a code, then `relay` connects to
it. That call is on a worker thread and posts its answer back through a channel,
because **`renzora_net::fetch` blocks the calling thread and must never run in a
system** — a system runs inside the frame the network pump needs to make
progress.

### Known inefficiency

The server understands a broadcast target; the client never uses it. A message
for every guest goes once per guest link, so a host with two guests uploads a
scene snapshot twice — on a home connection, in the scarce direction. Fixing it
means the session sending past the per-peer links rather than through them.

## Leases

Selecting claims. The host arbitrates by the simplest rule that works: first
come, first served, and a request for something already held is trimmed rather
than refused (a guest that asked for five and got four should carry on with the
four). It is a **social** lock — the editor does not refuse to move a claimed
object, it shows whose it is and lets the owner's version win. Enforcement would
mean auditing every mutation path, which is the cost state replication exists to
avoid.

## Files

Manifest first, bytes on request: the host sends path/size/content-hash, the
guest asks for what it lacks. A project is mostly large binaries that rarely
change, so a second visit moves almost nothing. The hash is FNV-1a — it decides
whether to *transfer* a file, never whether to *trust* one.

Two rules about writing to another machine's disk:

1. **Every path is resolved inside the project root and rejected if it lands
   outside**, checked structurally (no `..`, no absolute paths, no Windows path
   prefixes) rather than by `canonicalize`, which fails on files that do not
   exist yet. Both sides validate — the receiver against writing outside the
   project, the sender against reading outside it.
2. **The transfer is opt-in.** The guest sees what would be written and how much
   before anything moves.

Nothing here ever deletes. A file the host lacks is left alone.

## Framing — and never putting a timeout on the reader

A frame is `[magic: 4][len: u32 LE][bincode payload]`, assembled into one buffer
and written with a single `write_all`. Multiple writes per frame would risk
failing between the header and the payload, leaving a header on the stream with
nothing behind it.

**The reader blocks indefinitely and must keep doing so.** `read_exact` does not
report how many bytes it consumed before failing, so *any* early return that can
land mid-frame silently eats part of one and every frame after it parses from the
wrong offset. Two separate mistakes hit this in the first live session:

1. **The accepted socket was non-blocking.** The listener is non-blocking so its
   accept loop can poll for shutdown, and on Windows an accepted socket
   **inherits that flag**. Every `read_exact` returned `WouldBlock` immediately.
   `spawn_link` now calls `set_nonblocking(false)` on each accepted stream.
2. **A 500 ms read timeout** was set so the reader could poll its shutdown flag
   between frames. Removed.

The reader swallowed `WouldBlock`/timeouts and retried, which is what turned them
into corruption. In the session log it looked like:

```
piano disconnected: read failed: peer announced a 1560347651-byte frame
```

— a fault on the *reading* side, reported as the sending side's fault, with a
number that meant nothing. The tell was that repeated failures differed in a
single byte (`03 04 01 5D`, `03 03 01 5D`, …): payload data being read as a
length. Removing the swallow turned the same fault into an honest
`os error 10035` (`WSAEWOULDBLOCK`), which named the real cause immediately.

Shutdown is done by closing the socket instead, via a `hangup` closure the
transport installs on the `Link`. It is `Shutdown::Read` and not `Both`, because
a link is often dropped with something still queued — rejecting a peer sends
`Rejected` and drops the link in the same breath — and closing the write half
would turn an explained refusal into a bare disconnect.

`FRAME_MAGIC` is redundant on a correct stream and is there anyway: it turns the
same corruption, if anything ever reintroduces it, into "stream desynchronised"
at the first bad byte. `tests/protocol.rs` covers both.

## Protocol compatibility

`PROTOCOL_VERSION` is compared in the handshake and a mismatch is refused
outright, before anything else is read. A session that half-understands its peer
corrupts the project on disk, which is worse than failing to connect. Bump it for
any change that is not a pure append at the end of `CollabMsg` — **and for any
change to the framing**, which is equally incompatible and has no version of its
own.

The port is bound on `0.0.0.0`, so the first frame from a stranger must not be
able to do more than get itself hung up on. Frame lengths are validated before
the buffer behind them is allocated.

## Extending it

**A new synced subsystem** (terrain, tilemaps, code) is a new `CollabMsg`
variant plus a handler in `sync::apply_inbox`'s dispatch. Add at the end of the
enum and bump `VERSION_MINOR`-style only if the shape is a pure append.

**A relay transport** is a new module beside `tcp` that fills a `Link` from a
WebSocket instead of a socket. Nothing above `link` changes.

## Tests

- `tests/protocol.rs` — framing round trip, truncation, oversized-frame refusal,
  id partitioning, path-traversal refusal.
- `tests/round_trip.rs` — two real worlds: upsert patches in place rather than
  duplicating, unseeded upsert spawns, repeated applies converge, `ExactSet`
  excludes children, parent links follow the seed.
- `relay.rs` unit tests — the envelope round trip. A wrong endianness there
  would not fail loudly; it would route to peer 50331648 instead of peer 3 and
  lose the message.
- `tests/loopback.rs` — a real listener and a real client over a real socket.
  This is the one that matters for the transport: every framing bug so far lived
  in the gap between "the codec round-trips against a `Vec<u8>`" (always true)
  and "the codec round-trips through an *accepted socket*" (was not). It sends a
  4 MB frame followed by twenty small ones and checks all twenty-one arrive
  intact and in order, holds a link idle for 1.5 s, and checks that a message
  queued immediately before a hangup still arrives.

Run with `cargo test --profile dist -p renzora_collab`. The whole suite is ~2 s.

## Not done yet

- Terrain, tilemap, material/blueprint graphs and script buffers do not sync.
- File changes after the initial sync are not pushed automatically; the
  `FileTouched` message and its handler exist but nothing sends it yet.
- A direct connection is unencrypted. A relayed one is WSS end to end, but the
  relay is a third machine in the middle.
- Undo is per-peer; there is no shared timeline.
- The relay client does not use the server's broadcast target (see above).
- Rooms are in-memory, so a website deploy ends every live session.
