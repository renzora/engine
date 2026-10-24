# Physics

Add rigid bodies, colliders, joints and forces to your game through the `renzora_physics` API: one set of components for **3D** scenes and **sprite (2D)** scenes alike, simulated by whichever physics engine the project picks.

## Physics engines

Renzora's physics API lives in `renzora_physics`, and the simulation is a backend behind it. The bundled backend, the `avian_physics` plugin compiled into the engine, runs **[Avian](https://github.com/Jondolf/avian)**: the `avian3d` crate for the 3D world and `avian2d` for the 2D one. `PhysicsPlugin` is part of the engine's foundation (installed by `add_engine_plugins`), so physics is available in the editor, in a shipped game, and on the headless server. A build that leaves avian out (see [Physics backends](../extending/physics-backends.md)) keeps the API and simulates nothing until another engine is installed. The two worlds coexist in one app and never touch each other's bodies.

- In the **editor**, the companion `renzora_physics_editor` pauses the simulation at startup so your scene sits still until you press play (both worlds pause and resume together).
- In a **shipped game** the simulation runs immediately.

Another engine installs as a plugin. The **Rapier Physics** plugin adds `rapier3d` and `rapier2d`; pick one for each world in **Project Settings > Physics**. Everything on this page works the same on either engine, and the console says which one runs each world once the project has loaded (`3D physics: rapier3d, 2D physics: avian2d`). Writing your own engine's backend is covered in [Physics backends](../extending/physics-backends.md).

### Which world does an entity go to?

You never pick a world by hand: each entity is routed **once**, when its physics components are first wired up.

- It's a **2D entity** if it has a `Sprite`, sits anywhere under a `Node2d`, or carries the explicit `Physics2d` marker component. Painted tiles and 2D props all hit these rules naturally.
- **Everything else** goes to the 3D world.

`Physics2d` is for the rare 2D physics entity with no visual of its own: an invisible trigger area, or the merged colliders a [tilemap layer generates](/docs/r1-alpha8/editor/tilemap). The 2D world maps the shared shape data onto the plane: **Box** → rectangle (from `half_extents` X/Y), **Sphere** → circle, **Capsule** → 2D capsule, **Cylinder** → circle; a **Mesh** shape has no 2D source geometry and falls back to the fitted box. Of the axis locks, `lock_rotation_z` is the one that matters in 2D (it locks *the* rotation); use it on characters so collisions don't tip them over. Gravity, time scale and substeps from the [world physics settings](#world-physics-settings) apply to the 2D world too (2D gravity is the vector's X/Y).

## Physics components

You author physics with **two serializable, engine-independent components**. They save to scenes as plain data, and the selected physics engine builds its own bodies and colliders from them when the entity becomes active.

- `PhysicsBodyData`: how the body moves (mass, damping, gravity, axis locks, CCD).
- `CollisionShapeData`: the collision geometry, surface properties and layers.

An entity can have either or both. A `RuntimePhysics` marker is added once it is wired up; changing the data at runtime re-applies it, and removing it takes the entity out of the simulation.

### Rigid body: `PhysicsBodyData`

| Body type (editor label) | Behavior |
|---|---|
| **Rigid Body** | Affected by gravity and forces. Has mass, falls, bounces, gets pushed. |
| **Static Body** | Never moves. Floors, walls, immovable scenery. |
| **Kinematic Body** | Code-controlled. Pushes dynamic bodies but ignores forces. Moving platforms, characters. |

| Field | Default | Meaning |
|---|---|---|
| `mass` | `1.0` | Heavier bodies push lighter ones. |
| `gravity_scale` | `1.0` | `0` = floats, `1` = normal, `2` = double gravity. |
| `linear_damping` | `0.0` | Resists linear motion (drag). |
| `angular_damping` | `0.05` | Resists spin. |
| `lock_rotation_x/y/z` | `false` | Freeze rotation on an axis. |
| `lock_translation_x/y/z` | `false` | Freeze movement on an axis. |
| `ccd` | `false` | **Continuous (CCD)** in the inspector. Sweeps the body between steps so a fast one (a bullet, a thrown ball) can't pass through a thin wall in one step. Costs a little per step, so leave it off for ordinary bodies. |

### Collider: `CollisionShapeData`

| Shape (editor label) | Notes |
|---|---|
| **Box** | Sized by `half_extents` (a `Vec3`). Crates, walls, platforms. |
| **Sphere** | Sized by `radius`. Balls, projectiles. |
| **Capsule** | `radius` + `half_height`. Characters. |
| **Cylinder** | `radius` + `half_height`. Columns, barrels. |
| **Mesh** | Trimesh built from the entity's `Mesh3d` once the mesh asset loads. Size fields don't apply. |

| Field | Default | Meaning |
|---|---|---|
| `offset` | `0,0,0` | Local offset of the shape from the entity origin. |
| `rotation` | none | Turn of the shape about its offset point (shown in degrees in the inspector). A capsule lying along a bone, or a box set at an angle, without a child entity to hold the turn. Not used by Mesh shapes. |
| `friction` | `0.5` | `0.0` (ice) to high (grippy). |
| `restitution` | `0.0` | Bounciness: `0.0` (no bounce) to `1.0` (full bounce). |
| `is_sensor` | `false` | Pass-through trigger: detects overlap without blocking. |
| `collision_layers` | layer 0 | **Layers** in the inspector. The layers this collider is on, one bit per layer (32 of them). |
| `collides_with` | all | **Collides With** in the inspector. The layers it collides with. |

> A **Mesh** collider is a concave trimesh and is best used on **static** geometry. For moving bodies prefer a primitive shape (box/sphere/capsule/cylinder).

#### Collision layers

Two colliders touch only when **each** is on a layer the other collides with, so either side can opt out. Put the player on layer 1 and pickups on layer 2, then untick layer 1 from a ghost wall's **Collides With** to let the player through while everything else still bumps into it. The inspector shows layers 0 to 7 as checkboxes; the other 24 are there for code to use and survive editing.

## Joints

A **joint** ties two bodies together. It lives on an entity of its own (`PhysicsJoint`), so a body can have any number of them, and despawning the joint's entity is how a joint breaks. Joints are built at runtime by code (a ragdoll, a door, a rope), not saved in scenes.

| Kind | What the bodies can do |
|---|---|
| `Fixed` | Nothing: welded together. |
| `Spherical` | Turn any way about the anchor (a ball and socket), optionally within a cone (`swing_limit`) and a twist limit. 3D only. |
| `Revolute` | Turn about one axis only (a hinge), optionally between two angles. In 2D it turns about Z. |
| `Prismatic` | Slide along one axis only, optionally between two distances. |
| `Distance` | Stay between `min` and `max` apart (a rope or a rod), otherwise free. |

Each joint has an anchor on each body (in that body's local space), a `compliance` (0 = rigid; higher bends further while the anchors stay together), and `collide_connected` (whether the two bodies still collide with each other).

```rust
use renzora::physics::{JointKind, PhysicsJoint};

// A lamp hanging from a ceiling hook on a 2 m chain.
commands.spawn(
    PhysicsJoint::new(hook, lamp, JointKind::Distance { min: 0.0, max: 2.0 })
        .with_collide_connected(false),
);
```

[Ragdolls](/docs/r1-alpha8/scripting/ragdoll) build their skeletons out of spherical joints this way.

## Adding physics in the editor

Select an entity, then in the **Inspector** use **Add Component**:

1. Add a **Physics Body** (inserts `PhysicsBodyData`). This also drops in a default **Collision Shape** automatically, auto-fitted to the entity's mesh bounds, unless the entity (or one of its children) already has one, so compound bodies built from child shapes are left alone.
2. Optionally adjust the **Collision Shape** (`CollisionShapeData`): pick a different shape from its dropdown, turn it, set its layers, or add more shapes on child entities for a compound body.

The collision-shape card has an **Edit** toggle (collider edit mode) that hides the transform gizmo so you can resize/move the collider directly. In the **2D viewport** the same toggle shows a green collider frame with eight drag handles: drag a handle to resize (boxes resize from the grabbed edge; circle and capsule radii resize about the centre), drag inside the shape to move its offset, for example to shrink a tree object's collider down to its trunk base. Each gesture is one undo step. While the toggle is on, clicks edit the collider instead of selecting/moving sprites; toggle it off to get the normal picker back.

## World physics settings

Global simulation settings live in `PhysicsPropertiesState`, edited from the editor's physics controls. They are world-level (not per-entity) and are **not** exposed to scripts.

| Setting | Default | Notes |
|---|---|---|
| Gravity preset | **Earth** | Earth `-9.81`, Moon `-1.62`, Mars `-3.72`, Jupiter `-24.79`, Zero-G `0`, or Custom (m/s² on Y). |
| Gravity vector | `0, -9.81, 0` | Set directly for arbitrary directions; switches the preset to Custom. |
| Time scale | `1.0` | Slows or speeds the whole simulation. |
| Substeps | `6` | Solver substeps per step (higher = more stable, more cost). |

## Scripting physics

Physics script helpers don't talk to a physics engine directly: they push `ScriptAction`/`ScriptCommand` events that `renzora_physics` turns into commands for whichever engine simulates the **script's own entity**. The named helpers are sugar over those verbs.

### Forces and velocity

| Function | Effect |
|---|---|
| `apply_force(x, y, z)` | Applies a force for the current frame (cleared every frame: call it each `on_update` for a sustained push). |
| `apply_torque(x, y, z)` | The same for spin: a torque about each world axis for the current frame. |
| `apply_impulse(x, y, z)` | Sets the body's linear velocity to this value, as a kick. (Kept as it always was, so existing scripts behave the same; it is not a true impulse.) |
| `apply_angular_impulse(x, y, z)` | Spins the body by an instantaneous angular impulse. |
| `set_velocity(x, y, z)` | Directly sets the linear velocity. |
| `set_linear_velocity(x, y, z)` | Alias for `set_velocity` (registered by the physics extension). |
| `set_angular_velocity(x, y, z)` | Directly sets the spin, in radians per second about each world axis. |
| `set_gravity_scale(scale)` | Adjusts this body's gravity scale at runtime. |
| `move_controller(x, y, z)` | Kinematic collide-and-slide move (see below). |

```lua
-- thruster.lua (Dynamic rigid body)
function on_update()
    -- continuous upward push while held; re-applied every frame
    if is_key_pressed("Space") then
        apply_force(0.0, 50.0, 0.0)
    end

    -- instantaneous velocity change (a "jump kick")
    if is_key_just_pressed("E") then
        apply_impulse(0.0, 10.0, 0.0)
    end

    -- spin like a top
    if is_key_just_pressed("R") then
        set_angular_velocity(0.0, 10.0, 0.0)
    end
end
```

> **2D bodies:** the force and velocity functions route to the entity's own world automatically. On a 2D body they apply the X/Y of a linear vector and the Z of a spin, so the same script works on a sprite. `move_controller` is 3D-only for now; drive 2D characters with `set_velocity` (lock rotation on the body so collisions don't spin it).

### Kinematic character movement

For a **Kinematic Body** character, use `move_controller(dx, dy, dz)` (Lua). It performs a **collide-and-slide** sweep of the collider by that delta, stops at walls, slides along them, and updates the grounded state. Under the hood it fires the `kinematic_slide` action.

```lua
-- character.lua (Kinematic Body with a capsule collider)
function props()
    return {
        speed   = { value = 5.0, hint = "Move speed (units/s)" },
        gravity = { value = 18.0, hint = "Fall acceleration" },
        _vy     = { value = 0.0,  hint = "Internal: vertical velocity" },
    }
end

function on_update()
    -- horizontal from input axes
    local dx = input_x * speed * delta
    local dz = input_y * speed * delta

    -- simple gravity / ground handling
    local grounded = get("PhysicsReadState.grounded")
    if grounded then
        _vy = 0.0
        if is_key_just_pressed("Space") then
            _vy = 7.0
        end
    else
        _vy = _vy - gravity * delta
    end

    move_controller(dx, _vy * delta, dz)
end
```

The slide accepts an optional `max_slope` (degrees, default `55`) when called as a raw action:

```lua
action("kinematic_slide", { x = dx, y = dy, z = dz, max_slope = 45.0 })
```

### Reading physics state

Per-entity physics is mirrored into a reflect-readable `PhysicsReadState` component, refreshed every frame. Read it with `get("PhysicsReadState.<field>")`.

| Read | Type | Meaning |
|---|---|---|
| `get("PhysicsReadState.grounded")` | bool | A downward sweep found ground this frame (within `max_slope`). |
| `get("PhysicsReadState.velocity")` | vec3 | Linear velocity (world space). |
| `get("PhysicsReadState.speed")` | float | Magnitude of `velocity`. |
| `get("PhysicsReadState.ground_normal")` | vec3 | Contact normal of the last ground hit (`0,1,0` when airborne). |

For touch/overlap, the `is_colliding` context global is `true` whenever the entity has any active collision this frame:

```lua
function on_update()
    if is_colliding then
        print_log("touching something")
    end
end
```

`CollisionReadState` (`entered`, `exited`, `entered_name`, `exited_name`, `colliding`) reports the frame a contact began or ended; [Blueprints](/docs/r1-alpha8/scripting/blueprints) expose it as the `on_collision_enter` / `on_collision_exit` events.

> `grounded` and `ground_normal` are written by the kinematic slide, so they're meaningful for entities you drive with `move_controller`.

### Collision events in Rust

Engine code and Rust scripts can observe collisions directly. The engine triggers `CollisionStarted` and `CollisionEnded` on each body as contacts begin and end, whichever engine is simulating. A start carries the first contact: where it is, the normal pointing at the body, and how hard the two met (the normal impulse, which spikes on a landing; useful for impact sounds).

```rust
use renzora::physics::{CollisionStarted, PhysicsContacts};

app.add_observer(|hit: On<CollisionStarted>| {
    let c = hit.contact;
    if c.impulse > 5.0 {
        info!("{:?} hit {:?} hard at {}", hit.entity, hit.other, c.point);
    }
});
```

`PhysicsContacts` on a body lists everything it touches right now, with the same detail.

### Casting and overlap queries from Rust

`PhysicsBackends::cast` / `cast_one` sweep a ray, a shape, or an entity's own collider through the world, and `PhysicsBackends::overlap` asks which colliders a shape touches where it stands. Both take `&World`, so an exclusive system or a command closure can call them, and both go to whichever engine simulates the world you ask about.

```rust
use renzora::physics::{Cast, PhysicsBackends, PhysicsDim};

let hit = PhysicsBackends::cast_one(
    world,
    Cast::ray(PhysicsDim::D3, eye, Vec3::NEG_Y, 10.0).excluding([me]),
);
if let Some(h) = hit {
    info!("ground {} below, normal {}", h.distance, h.normal);
}
```

### What is *not* a script function

These appear in older docs or the internal `ScriptCommand` enum but have **no named text-script binding**. Don't call them, they won't resolve:

- `raycast`, `raycast_down`: there is no raycast text function (cast from Rust instead, above).
- `apply_impulse_to`, `find_entity_by_name`: no such helpers.
- `collisions_entered`, `collisions_exited`, `active_collisions`: these counters don't exist; use the `is_colliding` global or `CollisionReadState`.
- `on_collision`: there is **no collision lifecycle hook** for text scripts. Collision *events* (`on_collision_enter` / `on_collision_exit`) exist as [Blueprint](/docs/r1-alpha8/scripting/blueprints) nodes, and as `CollisionStarted` / `CollisionEnded` for Rust.

## Related

- [Parkour & Traversal](/docs/r1-alpha8/scripting/parkour): a character controller that owns movement outright, adding vaults, mantles, ledge hangs, ladders, wall runs and swings on top of the same collide-and-slide primitive
- [Ragdoll](/docs/r1-alpha8/scripting/ragdoll): skeletons built from bodies and spherical joints
- [Scripting Overview](/docs/r1-alpha8/scripting/overview): how scripts attach and run
- [Blueprints](/docs/r1-alpha8/scripting/blueprints): collision and physics nodes for visual scripting
- [Physics backends](/docs/r1-alpha8/extending/physics-backends): plugging in another physics engine
