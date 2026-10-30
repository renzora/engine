# Physics backends

The engine ships a physics **API** and no solver. What simulates is a backend
behind a Rust trait, the way a mixer or a scripting language is. See
[Audio backends](./audio-backends.md) and [Script backends](./script-backends.md),
whose shape this mirrors.

The bundled backend is the `avian_physics` plugin: an ordinary plugin, written
the way any installed one is, that the engine ships in `plugins/avian_physics`
and **links into the binary** (see
[Bundled plugins](./native-plugins.md#bundled-plugins)). It registers one backend
per avian crate, `avian3d` for the 3D world and `avian2d` for the 2D one, and is
present whenever the engine is built with `physics_3d` or `physics_2d`. The trait
they register through is public, so another plugin can bring a different engine.
The `rapier_physics` plugin does, for both worlds.

Physics stays removable. A lean export of a project with no physics leaves avian
out, and so does one whose Project Settings pick another engine for every world
with physics (the export's **Avian physics engine** toggle, the runtime's
`avian` feature). A checkout that wants no avian at all deletes `plugins/avian_physics`
(or runs `cargo renzora remove avian_physics`). The next build then has the API
and no engine, and the Console says that no physics engine is installed.

Everything in the engine that uses physics goes through this API, and nothing
else: ragdolls, parkour, the navmesh's collider obstacles, `move_controller`,
particle collisions, water buoyancy and the MCP plugin's colliders all work on
whichever backend a project picks. The contract crate no longer re-exports
avian (`renzora::avian3d` is gone); a plugin that wants physics uses
`renzora::physics`.

## What each side owns

| | |
|---|---|
| **The engine** (`renzora_physics` + `renzora::physics`) | the components scenes save, routing each entity to the 2D or 3D world, building collider geometry (including reading mesh assets), script actions, the read-back state scripts see, collision events, world settings, collide-and-slide, and the scene queries particles use |
| **The backend** | bodies, colliders, joints, the solver and its clock, contacts, and answering casts and overlaps |

A backend never reads a mesh asset, never decides what a cylinder means in 2D,
never decides when a collision "starts", and never implements a character
controller. Every backend would otherwise repeat that work and get its edge
cases wrong in its own way.

### Renzora components say what exists; the backend owns motion

This is the rule that lets very different engines sit behind one contract.
Avian keeps its state as Bevy components and maps ours onto its own; an engine
with its own world (Rapier, Jolt) keeps a map from `Entity` to its handles.
From above, they look the same.

What the engine writes, and a backend reads:

| Component | Meaning |
|---|---|
| `RuntimePhysics` | the entity is wired up for physics; mirror it |
| `RuntimePhysics2d` | it belongs to the 2D world (decided once, from its ancestry) |
| `PhysicsBodyData` | the body: type, mass, gravity scale, damping, axis locks, `ccd`. Watch `Changed<PhysicsBodyData>` |
| `ResolvedCollider` | the collider, already built: a `ResolvedShape` plus `offset`, `rotation`, friction, restitution, `sensor` and `layers`. Watch `Changed<ResolvedCollider>` |
| `PhysicsJoint` | a joint, on an entity of its own, between `body1` and `body2`. Watch `Changed<PhysicsJoint>`; the world is `body1`'s |

What removals mean (the engine does the bookkeeping that makes these the only
cases a backend sees):

| Removed | Do |
|---|---|
| `RuntimePhysics` | drop everything of the entity's: body, collider |
| `PhysicsBodyData`, entity still wired | drop the body; its collider stays, as fixed scenery |
| `ResolvedCollider` | drop the collider |
| `PhysicsJoint` | drop the joint. Despawning a joint's entity is how it breaks |

What a backend writes, and the engine reads:

| Component | Meaning |
|---|---|
| `BodyVelocity` | linear and angular velocity, world space. Inserted on every body by the engine; the backend fills it in |
| `PhysicsContacts` | what this body touches right now, as `Contact`s (the other entity, the deepest point, the normal pointing at this body, the normal impulse), rewritten every frame |

Contacts are state rather than enter/exit events on purpose: a backend that
missed one event would leave a body "touching" forever, and a set rewritten
every frame cannot drift. The engine diffs it every frame and triggers
`CollisionStarted` and `CollisionEnded` (entity events, with the first
contact on a start), so those mean the same on every backend.

### Collider shapes, layers and rotation

`ResolvedShape` is one of `Cuboid`, `Sphere`, `Capsule` (upright, by radius and
half height), `Cylinder`, `TriMesh` and `Heightfield` in 3D, and `Rectangle`,
`Circle` and `Capsule2d` in 2D. A backend builds it at the entity's world scale,
placed at `offset` and turned by `rotation` relative to the entity (2D reads only
the turn about Z).

`Heightfield` is a grid of heights (`HeightfieldData`: `columns` samples along X
by `rows` along Z, stored row by row, spanning `0..size` from the entity's
origin). Terrain chunks collide as one: it builds and queries far faster than the
same ground as a trimesh. A backend without a native heightfield can build
`HeightfieldData::triangles()` instead, which is the same surface. A mesh
collider whose entity carries a `HeightfieldSource` is resolved from it rather
than from the entity's `Mesh3d`.

`layers` is a `CollisionLayers` pair: the layers a collider is on
(`memberships`) and the ones it collides with (`filters`). Two colliders touch
only when each one's memberships meet the other's filters, which is exactly
avian's and Rapier's own rule, so map it straight across.

### Joints

`JointKind` is `Fixed`, `Spherical` (twist axis, optional swing and twist
limits; 3D only), `Revolute` (axis, optional angle limits), `Prismatic` (axis,
optional distance limits) or `Distance` (min and max). Anchors are in each
body's local space. `compliance` softens the joint's free directions and limits
while the anchors stay pinned; a backend whose joints soften some other way may
leave them rigid. `collide_connected` says whether the two bodies still collide.

A joint can arrive before its bodies are built (a ragdoll spawns them in the
same frame). Keep it pending and build it when they exist.

### When things happen

A backend adds ordinary systems to the `PhysicsSet`s. The step itself is its
own business (avian steps in `FixedPostUpdate`); everything here happens
between steps.

| Set | Schedule | Who | What |
|---|---|---|---|
| `Prepare` | `Update` | engine | route new entities, resolve colliders, handle removed data |
| `Sync` | `Update` | backend | create, update and drop bodies, colliders and joints |
| `Writeback` | `Update` | backend | write `BodyVelocity` and `PhysicsContacts` |
| `Read` | `Update` | engine | fill `PhysicsReadState` and `CollisionReadState`, trigger collision events |
| `Commands` | `PostUpdate` | backend | apply `PhysicsCommands` |

Commands are applied in `PostUpdate` because scripts run in `Update` in no fixed
order against these sets. Applying them any earlier would let a force pushed by
a late script miss the next step.

### Commands and settings

Scripts and engine crates push `PhysicsCommand`s into the
`PhysicsCommands` resource. Every backend reads the queue and acts on the
entities in its own world; the engine empties it straight after.

| Command | Meaning |
|---|---|
| `AddForce`, `AddTorque` | act through every step until the next frame's commands land. Pushed every frame it is a steady force at any frame rate; several in one frame add up |
| `AddImpulse`, `AddAngularImpulse` | instantaneous: velocity changes by impulse / mass (or inertia) |
| `SetLinearVelocity`, `SetAngularVelocity` | set outright. A 2D body reads the XY of a linear one and the Z of an angular one |
| `Strip` | drop the backend's body and collider while the entity keeps its data. Terrain streams far chunks out this way, and back in by re-inserting their shape |

`PhysicsSettings` (gravity, time scale, substeps, paused) is the engine's, and
every backend applies it in `PreUpdate`. That timing matters: it runs before the
fixed-step loop, so a pause written during one frame stops the very next step,
and the editor never nudges a scene by one tick as it opens.

### Queries

Two, both batched and both answered with read-only access to the world:

- **`cast`** answers `Cast`s: a ray, a `ResolvedShape`, or an entity's own
  collider (placed by the entity's pose; the backend adds the collider's own
  offset and turn), swept along a direction, with an exclusion list, a layer
  filter, a choice about sensors and about shapes already overlapping at the
  start. Build one with `Cast::ray` / `Cast::shape` and the builder methods.
- **`overlap`** answers `Overlap`s: which colliders a shape, held still at a
  pose, touches. It has a default that finds nothing, so a backend can leave it
  for later.

Callers use `PhysicsBackends::cast`, `cast_one` and `overlap`, which route each
query to the backend simulating its world. Read-only (`&World`) is deliberate:
a character controller asks what it hit between sweeps, and holding other
read-only queries open across its casts is only possible because a cast takes
no `&mut`. The engine's own users are collide-and-slide (`move_controller`),
the parkour controller, and the particle system's scene queries.

### Scene saving

A backend that keeps components on entities (avian does) lists their
type-path prefixes in `runtime_type_prefixes` (`"avian3d::"`). Scene saving
leaves them out, since they are rebuilt from the renzora data on load and a
scene carrying them would load only where that backend is installed. Data on an
entity marked `RuntimeOnlyPhysics` (a ragdoll's bones) is left out too, because
it is generated.

## Writing one

```rust
use bevy::prelude::*;
use renzora::physics::*;

struct MyEngine;

impl PhysicsBackend for MyEngine {
    fn name(&self) -> &str { "my_engine" }
    fn caps(&self) -> Caps { Caps::DIM_3D.union(Caps::TRIMESH).union(Caps::JOINTS) }
    fn cast(&self, world: &World, batch: &mut CastBatch) { /* answer the D3 casts */ }
    fn overlap(&self, world: &World, batch: &mut OverlapBatch) { /* optional */ }
    fn detach(&self, world: &mut World, dim: PhysicsDim) { /* drop your whole world */ }
}

pub struct MyPhysicsPlugin;

impl Plugin for MyPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_physics_backend(MyEngine);
        let active = physics_backend_selected("my_engine", PhysicsDim::D3);
        app.add_systems(Update, sync.in_set(PhysicsSet::Sync).run_if(active.clone()));
        app.add_systems(Update, writeback.in_set(PhysicsSet::Writeback).run_if(active.clone()));
        app.add_systems(PostUpdate, apply_commands.in_set(PhysicsSet::Commands).run_if(active));
        // ...and your own step, wherever your engine wants it.
    }
}

renzora::plugin!(MyPhysicsPlugin, Runtime);
```

Gate every system on `physics_backend_selected`, so an unselected backend costs
nothing per frame, and implement `detach` (see below), so it lets go of its world
when the project moves to another. Handle the removals in the table above: avian
gets most of them for free because its state lives on the entity, and an engine
with its own world does not.

`Caps` is answered honestly, as in audio and networking: `DIM_3D`, `DIM_2D`,
`TRIMESH`, `JOINTS`, `CCD`.

### Several register; the project picks one per world

Unlike audio or networking, where a second mixer would fight the first over one
device, every registered physics backend is kept, because two engines conflict
only if both run. The 3D and 2D worlds are chosen independently, in
**Project Settings > Physics**, which writes `project.toml`:

```toml
[physics]
backend_3d = "rapier3d"   # leave it out for the default
backend_2d = "rapier2d"
```

The default is the first backend registered for that world, which is the
bundled avian on any build that has it. A name whose plugin is not installed
logs a warning and keeps the default, so a project opened on a machine without
the plugin still has physics. Once the project's choice is applied, the log
says which engine runs each world: `[Physics] running 3D on `rapier3d`, 2D on
`avian2d``.

**Switching a live world** is `renzora_physics::switch_backend`, and the editor
does it the moment the setting changes: the old backend's `detach` throws away
its bodies, colliders and joints, every entity of that world loses its wiring
markers, every joint is marked changed, and the next frame routes each one to
the new backend as if the scene had just loaded. The play-mode game is its own
process, so it reads the choice from `project.toml` when it starts. One
consequence worth knowing: a terrain chunk that was streamed out is rebuilt by a
switch, and streams out again once the camera is far enough away.

A backend's `detach` must leave nothing running. Avian strips its parts from
every entity and pauses its own clock; the engine marks `PhysicsSettings`
changed on every switch, which is how a re-selected backend learns to unpause.

## The bundled backend

`plugins/avian_physics` maps the contract onto avian: `PhysicsBodyData`
onto `RigidBody`, `Mass`, `SweptCcd` and the rest, `ResolvedCollider` onto
`Collider`, `Friction`, `Restitution`, `Sensor` and `CollisionLayers`,
`PhysicsJoint` onto avian's joint of the same kind, commands onto
`ConstantForce`, `ConstantTorque`, `Forces` and the velocity components, settings
onto `Gravity`, `Time<Physics>` and `SubstepCount`.

Things worth knowing about it:

- **A collider's offset and turn are baked into it** as a one-shape compound.
  Avian recomputes `ColliderTransform` every step (identity for a collider on
  its body's own entity), so an offset put there is lost.
- **A 2D collider with no body gets a static one.** avian2d's standalone collider
  tree produces no contacts, so world geometry (tile colliders, prop trunks)
  would be walked through. A static body is the same thing semantically.
- **Avian is built with `xpbd_joints`**, its joint solver. Without it avian
  accepts joints and ignores them.

## A second backend: Rapier

The `rapier_physics` plugin (in the marketplace, source in
`github.com/renzora/plugins`) is the worked example of a backend with a world of
its own, and registers two: `rapier3d` and `rapier2d`. Each keeps rapier's
`PhysicsWorld` in a resource and maps from `Entity` to body, collider and joint
handles, steps on the fixed clock as avian does, and writes dynamic bodies'
poses back into their `Transform`s. Read it beside `plugins/avian_physics` to see
the same contract met two very different ways.

Details from it that any such backend needs:

- **Teleports are told apart from its own writes.** It records the transform it
  last wrote to each body; a `Transform` that changed and does not match was
  moved by the editor or a script, and the body is placed there.
- **New entities are placed from their local transforms**, composed up the
  hierarchy, not from `GlobalTransform`, which is only propagated in
  `PostUpdate` and still reads identity for a scene spawned this frame.
- **A child body is written relative to its parent's new pose.** A ragdoll's
  bones are each other's children; composing a bone against its parent's
  `GlobalTransform`, which is a step old, would leave every bone a step behind
  the one above it.

Where Rapier and avian differ, by how the engines work rather than by anything
missing: Rapier joints are rigid (it softens joints with springs, not
compliance), a distance joint keeps only its maximum, and a ball joint's swing
limit is a box rather than a cone.
