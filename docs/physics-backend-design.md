# Physics backends: design

**Status: proposal, nothing built.** This is the design for making physics a
backend behind a contract, the way audio, networking and scripting already are
(see `docs/r1-alpha8/extending/audio-backends.md`,
`network-backends.md`, `script-backends.md`). It lives outside `docs/r1-alpha8/`
on purpose: that tree publishes to renzora.com, and an API that does not exist
yet must not appear there. When the work lands, this becomes
`docs/r1-alpha8/extending/physics-backends.md`.

## Goal

A plugin can supply the physics engine. Avian stays the bundled backend; a
second one (Rapier first, Jolt as the stretch) installs beside it, and a project
setting picks which one simulates. Every engine crate, script, blueprint and
native plugin talks to the contract, never to a backend, so switching changes
nothing above it.

Non-goals for this work: writing a renzora physics engine (it becomes "just
another backend" later, and nothing here assumes it exists), and changing how
physics *behaves*. The avian backend after the refactor must produce the same
simulation as today.

## Where we are today

Half of this already exists, which is why it is tractable.

**Already engine-agnostic:**

- `PhysicsBodyData` and `CollisionShapeData` (`renzora_physics::data`) are what
  scenes save. Avian components are spawned from them at runtime and scene save
  strips anything under `avian3d::` / `avian2d::`.
- `renzora::scene_query` is a working backend contract in miniature: a trait,
  a resource holding a `dyn` implementation, a batched cast taking `&mut World`.
  Particles use it to collide with the level without linking avian.
- Scripts reach physics through `ScriptAction`s (`apply_force`,
  `kinematic_slide`, ...) and read it through `PhysicsReadState` /
  `CollisionReadState`. Neither names avian.

**Still avian-shaped:**

| Where | What it touches |
|---|---|
| `renzora_physics/src/lib.rs` | inserts `ConstantForce` / `LinearVelocity` for script actions; pauses through `Time<Physics>`; kinematic slide reads `SpatialQuery` + `Collider` |
| `renzora_physics/src/read_state.rs` | reads `LinearVelocity` and `Collisions` |
| `renzora_physics/src/backend/avian*.rs` | the backend proper (fine: this is where avian belongs) |
| `renzora_parkour` (`drive.rs`, `probe.rs`, `lib.rs`) | `SpatialQuery` ray and shape casts, `Collider::capsule`, `Position` |
| `renzora_ragdoll` (`generate.rs`, `toggle.rs`) | spawns `SphericalJoint`s, flips `RigidBody` Kinematic/Dynamic |
| `renzora_navmesh` | `NavmeshUpdaterPlugin<avian3d::Collider, _>` for obstacles |
| `renzora_engine/src/scene_io` | deny list of avian types, `starts_with("avian3d::")` filters |
| `renzora_export/src/capabilities.rs` | scans scenes for `avian3d::` / `avian2d::` |
| `renzora/src/lib.rs:355` | `pub use avian3d; pub use avian2d;` for native plugins |

That last row is the important one. It makes avian part of the public plugin API:
any native plugin or `.rs` script that wrote `use renzora::avian3d::prelude::*`
only works while avian is the engine.

## The shape of the contract

### The decision everything else follows from

Avian keeps its state as Bevy components. Rapier and Jolt keep their own world
outside the ECS and want to be told what changed. A contract written in avian's
terms ("put these components on the entity") cannot host the other two; one
written in their terms (handles and a `step()` call) would force avian to
shadow its own ECS state for no reason.

So the contract is written in **renzora's** terms, and both kinds adapt to it:

- **Renzora components are the source of truth for what exists.** Bodies,
  colliders and joints are described by contract components. A backend never
  decides that a body exists; it mirrors what the components say.
- **The backend is the source of truth for motion.** After a step it writes poses
  and velocities back. Nothing else writes them except through the command queue.
- **Everything the engine asks of a backend that is not "simulate" goes through a
  trait**: queries, capabilities, teardown.

Avian's adapter maps contract components onto avian components (what
`spawn_physics_body` does today). Rapier's maps them onto rapier handles in a
`HashMap<Entity, RigidBodyHandle>`. Both look identical from above.

### Contract components (`renzora::physics`)

Moved into the contract crate, because a backend plugin must name them without
linking `renzora_physics` (CLAUDE.md §6.3).

| Component | Status | Notes |
|---|---|---|
| `PhysicsBodyData` | moves from `renzora_physics` | unchanged fields |
| `CollisionShapeData` | moves | unchanged fields |
| `Physics2d` | moves | unchanged |
| `PhysicsJoint` | **new** | `kind` (Fixed, Spherical, Revolute, Prismatic, Distance), the other body, both anchors, axis, limits, compliance. Ragdoll's `SphericalJoint` maps onto it one to one |
| `BodyVelocity` | **new** | linear + angular, written by the backend after each step; replaces reading `LinearVelocity` |
| `PhysicsReadState`, `CollisionReadState` | move | now filled from contract data, so the `_2d` twins in `read_state.rs` collapse into one system each |

**Moving a reflected type changes its type path, and scenes store type paths.**
Each moved type keeps its old path with `#[type_path = "renzora_physics::data"]`
(or an alias, per the scene type-path resilience rule), so every saved scene
still loads. This must be verified with an old `.bsn` before anything else lands.

### Resolved shapes: the engine builds geometry, the backend never loads assets

`CollisionShapeType::Mesh` currently makes the avian backend read
`Assets<Mesh>` itself (`init_avian_mesh_colliders`). Every backend would have to
repeat that, and get the TriangleList and index-format edge cases right again.

Instead the engine resolves each `CollisionShapeData` into a
`ResolvedShape`, and that is all a backend sees:

```rust
pub enum ResolvedShape {
    Cuboid { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    TriMesh(Arc<TriMeshData>),   // vertices + indices, built once per mesh asset
    // 2D
    Rectangle { half_extents: Vec2 },
    Circle { radius: f32 },
}
```

Terrain heightfields and convex hulls are added as variants when something needs
them, behind capability bits (below), so a backend that cannot do them says so.

### Commands: forces, impulses and teleports

Today `apply_force` inserts avian's `ConstantForce` and a cleanup system removes
it a frame later, and `apply_impulse` **sets the velocity**, which is not an
impulse (the comment in `handle_physics_script_actions` admits it). Both go away.

The engine collects a `PhysicsCommands` queue (`AddForce`, `AddImpulse`,
`AddTorque`, `SetLinearVelocity`, `SetAngularVelocity`, `Teleport`, `Wake`), and
the backend drains it at the start of each step. Script actions, blueprints and
engine crates all push into the same queue. Every backend has a real impulse, so
the fix comes for free.

### Contacts

The backend writes two contract messages after each step:

```rust
pub struct CollisionStarted { pub a: Entity, pub b: Entity }
pub struct CollisionEnded   { pub a: Entity, pub b: Entity }
```

`CollisionReadState` is then built from these by the engine, once, for both
dimensions. Contact points and impulses are a later, capability-gated addition;
nothing reads them today.

### The schedule and the clock

The engine owns the fixed timestep, pausing, time scale, gravity and substeps, as
a `PhysicsSettings` resource in the contract (today's `PhysicsPropertiesState`,
minus its avian-specific apply code). The editor's Physics panel and the pause on
entering edit mode then work identically on every backend.

Physics runs in an engine schedule with four sets. The engine owns the first,
and a backend adds its systems to the other three:

| Set | Who | What |
|---|---|---|
| `PhysicsSet::Prepare` | engine | resolve new or changed shapes, route 2D vs 3D (`auto_init_physics`, unchanged logic) |
| `PhysicsSet::Sync` | backend | create/update/remove bodies, colliders and joints; push kinematic and teleported poses; drain `PhysicsCommands` |
| `PhysicsSet::Step` | backend | simulate one fixed step (substeps are the backend's business) |
| `PhysicsSet::Writeback` | backend | write dynamic bodies' `Transform` and `BodyVelocity`; emit contact messages |

Avian's adapter installs `PhysicsPlugins::new(<engine schedule>)` so avian runs
inside `Step` on the engine's clock rather than its own `Time<Physics>`.

Poses are world space. A dynamic body under a parent has its `Transform` written
back relative to that parent's `GlobalTransform`, which is what avian does
already and what the others must be told explicitly.

### The trait

```rust
pub trait PhysicsBackend: Send + Sync + 'static {
    /// Shown in the backend picker and the logs.
    fn name(&self) -> &str;

    /// What this backend can do. The engine never asks for an unclaimed one.
    fn caps(&self) -> Caps;

    /// Install the backend's systems into `PhysicsSet::{Sync, Step, Writeback}`.
    /// Called for every registered backend; its systems are gated on it being
    /// the selected one, so an unselected backend costs nothing per frame.
    fn build(&self, app: &mut App);

    /// Throw away every internal body, collider and joint. Called when the
    /// project switches away from this backend; the engine then re-initialises
    /// every entity on the new one.
    fn detach(&self, world: &mut World);

    /// Ray, shape and overlap queries. See "Queries".
    fn query(&self, world: &mut World, batch: &mut QueryBatch);

    /// Type-path prefixes of this backend's runtime components, which scene save
    /// and the export scan must ignore. Avian answers `["avian3d::", "avian2d::"]`.
    fn transient_type_prefixes(&self) -> &[&str] { &[] }
}
```

`Caps` bits, answered honestly as in audio and networking: `DIM_3D`, `DIM_2D`,
`JOINTS`, `TRIMESH`, `CCD`, `HEIGHTFIELD`, `CONVEX`. A scene that uses a capability
the selected backend lacks gets a Problems entry naming the entity and the
feature, not a silently missing collider.

Registration mirrors the others:

```rust
app.add_physics_backend(MyBackend::default());
```

### Several register, one simulates (per dimension)

This is the one place physics deliberately differs from audio and networking,
which keep the first registration and refuse the rest. Here **every registered
backend is kept**, and the project chooses:

```toml
# project settings
[physics]
backend_3d = "avian3d"   # default: the bundled one
backend_2d = "avian2d"
```

That is what makes the switch possible without rebuilding the engine, and what
lets someone A/B two backends on the same scene. It is safe because an
unselected backend's systems never run, and switching is only allowed in edit
mode: `detach` the old one, clear `RuntimePhysics` on every entity, and the
normal init path rebuilds everything on the new one the next frame.

3D and 2D are selected independently, so Rapier for 3D and Box2D for 2D in one
project is a legitimate configuration.

## Queries

`renzora::scene_query` already has the right shape (batched, `&mut World`, one
cached system per batch), so queries grow out of it rather than replacing it:

- `SceneRay` gains a `shape` (ray, sphere, capsule, box), an `exclude` entity
  list and an `include_sensors` flag. Particles keep working unchanged: they
  send a sphere or a ray and no exclusions.
- Overlap tests ("what is inside this capsule") join as a second request kind.
- `SceneQueries` becomes a thin forwarder to the selected backend's `query`.

**Collide-and-slide moves to the engine.** `shape_cast_slide` is pure logic on
top of shape casts, so it is written once against the contract and every backend
gets `move_controller` for free. A backend never implements a character
controller.

**Open question: immediate queries.** Avian's `SpatialQuery` is a system
parameter any ordinary system can use. A `&mut World` batch is not: parkour's
probes (five ray casts, two shape casts per frame) would have to move into an
exclusive system or queue their casts and read the answers a frame later. The
alternative is a backend-provided read-only snapshot (`Arc<dyn QueryWorld>`
refreshed after each step) that ordinary systems read in parallel. That is
cheaper for callers but costs the avian adapter a copy of its BVH every step.
**Proposal: batch only in phase 1, measure parkour, decide after.**

## The avian re-export

`renzora::avian3d` / `renzora::avian2d` stay for now, behind the same features,
documented as **backend-specific and not portable**: a plugin that uses them only
works when avian is the selected backend. The contract is the portable path, and
the docs point there. Removing the re-export outright would break every
published plugin that builds a body today, so that is a deprecation for a later
version, not part of this work.

## What changes in each crate

| Crate | Change |
|---|---|
| `renzora` | new `physics` module: components, `ResolvedShape`, commands, messages, `PhysicsSettings`, `PhysicsBackend`, `Caps`, `add_physics_backend`; `scene_query` extended |
| `renzora_physics` | becomes the engine side: routing, shape resolution, commands, read state, settings, collide-and-slide, backend selection. Owns no avian code |
| `renzora_physics_avian` (**new**, split out of `renzora_physics/src/backend`) | the bundled backend, linked in by feature exactly as `renzora_audio_backend` is |
| `renzora_parkour` | `SpatialQuery` -> contract queries; `Collider::capsule` -> `ResolvedShape::Capsule` |
| `renzora_ragdoll` | `SphericalJoint` -> `PhysicsJoint`; `RigidBody` flip -> `PhysicsBodyData.body_type` |
| `renzora_navmesh` | obstacle source from `CollisionShapeData` (vleue_navigator is vendored, so its obstacle trait can be implemented for it) |
| `renzora_engine` scene_io | deny list and prefix checks come from `transient_type_prefixes` of every registered backend |
| `renzora_export` | the scan already keys on scene content plus 2D/3D; the `avian*::` strings become the selected backend's capability |
| `renzora_physics/editor` | Physics panel edits `PhysicsSettings`; gains a backend picker |

## Phases

1. **Contract, no behaviour change.** Add `renzora::physics`, move the data types
   (with their old type paths), split avian into `renzora_physics_avian`, route
   `renzora_physics` through the trait. Exit test: the existing physics tests and
   `avian2d_collision.rs` pass, and an old scene loads and plays identically.
2. **Move the consumers.** Parkour, ragdoll, navmesh, scene_io, export onto the
   contract. Exit test: `grep avian3d` outside `renzora_physics_avian` finds only
   the re-export.
3. **Selection.** Project setting, backend picker, `detach` and re-init on
   switch, Problems entries for unclaimed capabilities.
4. **Rapier native plugin** in `plugins/`, built against the staged SDK like any
   other. Exit test: a fixed set of scenes (box stack, ragdoll, parkour course,
   2D tile level, trimesh terrain) behaves the same on both. "The same" means
   settles to rest and nothing tunnels, not bit-identical.
5. **Jolt** (stretch). Proves the contract survives FFI and a C++ engine. The
   open risk is the build: a native plugin compiles on the installing machine,
   so a Jolt plugin needs a C++ compiler there, and the wasm, Android and iOS
   cross-builds need their own answer. Worth a spike before committing to it.

Docs (`extending/physics-backends.md`, `scripting/physics.md`) and
`RELEASE_NOTES.md` lines land with each phase, not at the end.

## Open questions for review

1. **Immediate queries**: batch only, or a snapshot? (Proposal: batch, measure.)
2. **The avian re-export**: keep as "not portable" for this version, deprecate
   next? (Proposal: yes.)
3. **Selection granularity**: per project is proposed. Per scene is possible but
   means a scene change can force a backend switch mid-session; no case for it
   yet.
4. **Interpolation**: avian interpolates poses between fixed steps; Rapier and
   Jolt do not. Should the engine interpolate `Transform` from the written-back
   poses once for every backend? (Proposal: yes, in phase 3, so the backends
   compare fairly.)
5. **Collision layers**: nothing uses avian's `CollisionLayers` today. Add a
   contract `CollisionGroups` now, or when the first feature needs it?
   (Proposal: when needed.)
