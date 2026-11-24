# State Replication

How the server keeps every client's copy of the world in step with its own.

## What replicates

An entity replicates when it has `Networked`. For each one the server sends:

- its **transform**, as the `NetworkTransform` on it says;
- every **registered component** it has (see below), whenever one changes;
- its **spawn** when it first appears and its **despawn** when it goes.

Nothing else. Children are not replicated one by one: a networked prefab is a `NetworkPrefab` path, and every peer instances the prefab from its own copy of the project. Meshes, materials and textures never cross the network.

## How it is sent

The server replicates up to 30 times a second (or the tick rate, if lower):

| What | Channel | When |
|---|---|---|
| Spawn, despawn, component changes | reliable, ordered | as they happen |
| Transforms | unreliable | when they moved, and at least once a second regardless |
| A late joiner's world | reliable, ordered | once, on joining |

Reliable messages arrive exactly once and in order, however many packets are lost; large ones (a spawn with big components, a late joiner's whole world) are split and reassembled. Unreliable transforms are not resent: a newer one replaces a lost one, and the once-a-second refresh repairs a final position that was lost.

## Scene entities and spawned entities

- **Placed in a scene.** Server and client load the same scene file, so both already have the entity. The server's spawn carries the entity's scene id, and the client adopts its own copy instead of making a second. If the client's scene has not finished loading, the spawn waits for it for a few seconds.
- **Spawned at run time** (by `renzora::network::spawn_prefab` or a player spawner). The client spawns a new entity, applies the components, and instances its `NetworkPrefab`.

When a client disconnects, what the network spawned on it is removed, and scene entities it adopted are released back to how the scene had them.

## `NetworkTransform`

| Field | Default | Effect |
|---|---|---|
| **Moved By** (`authority`) | `Server` | `Server`: the server moves it and clients follow. `Owner`: the owning client moves it, the server passes the moves on. |
| `interpolate` | `true` | Draw smoothly between received positions. Off: snap to each. |
| `sync_rotation` | `true` | Send rotation. |
| `sync_scale` | `false` | Send scale. |

An entity without one replicates translation and rotation, server-moved and interpolated.

### Interpolation

Remote entities are drawn 100 ms in the past, between the two received positions either side of that moment. That delay is what hides uneven arrival and the occasional lost packet. All peers share the server's clock: the server stamps what it sends, and clients estimate its clock from those stamps and their round trip.

### Owner-moved entities

With **Moved By: Owner**, the owner's client sends where the entity is (up to 30 times a second, when it moves). The server accepts it only from that owner and only for an entity that allows it, draws it interpolated (on a host), and sends it to everyone else. The owner is never sent its own entity's position back, so nothing drags it.

## Replicating your own components

Register the component's type, and it replicates on every networked entity that has it. It must be reflected as a component:

```rust
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct Health(f32);

// In a plugin:
use renzora::network::AppReplicateExt;
app.register_type::<Health>().replicate::<Health>();
```

The registry is also a resource, `renzora::network::ReplicationRegistry`, if a component needs registering later. Every peer must know the type: a client that has not registered it logs once and skips it.

Registered by default: `Name`, `NetworkOwner`, `NetworkPlayer`, `NetworkPrefab` and `NetworkTransform`. `Name` is handy: a server renaming an avatar to `"Ana - 3 pts"` is a scoreboard every client sees, late joiners included.

Each tick the server serializes every registered component and sends the ones whose value changed. That suits components that change now and then. For something that changes every frame, prefer an unreliable RPC.

## Ownership

`NetworkOwner` is `Server` for anything in a scene, and `Client(id)` for an avatar. When a player leaves, everything they own is despawned on every peer.

## Saving

`Networked`, `NetworkOwner`, `NetworkTransform` and `NetworkPlayerSpawner` save with the scene. `NetworkId`, which the server hands out per session, does not.

## Limits

- Every client is sent every networked entity (no interest management).
- No client-side prediction: something the server moves for a client (rather than its owner) reacts after a round trip.
- A replicated rigid body is still simulated on each client. Make client copies kinematic, or move such things by script on the server.
