# Physics

Add rigid bodies, colliders, and forces to your game with Avian, wrapped by the `renzora_physics` crate — **Avian 3D** for 3D scenes and **Avian 2D** for sprite scenes, behind one set of components.

## Backends: Avian 3D + Avian 2D

Renzora's physics live in `renzora_physics`, which wraps **[Avian](https://github.com/Jondolf/avian)** — both the `avian3d` and `avian2d` crates (0.6.1 line). The crate exposes a backend-agnostic API and converts your scene components into Avian's components at runtime. `PhysicsPlugin` is part of the engine's foundation (installed by `add_engine_plugins`), so physics is always available — in the editor, in a shipped game, and on the headless server. The two simulations coexist in one app and never touch each other's bodies.

- In the **editor**, the companion `renzora_physics_editor` pauses the simulation at startup so your scene sits still until you press play (both worlds pause and resume together).
- In a **shipped game** the simulation runs immediately.

> Avian is the engine's only physics backend. The default feature set is `["avian", "avian2d", "lua"]`; the data components below are backend-agnostic by design.

### Which backend does an entity get?

You never pick a backend by hand — each entity is routed **once**, when its physics components are first wired up:

- It's a **2D entity** (goes to Avian 2D) if it has a `Sprite`, sits anywhere under a `Node2d`, or carries the explicit `Physics2d` marker component. Painted tiles and 2D props all hit these rules naturally.
- **Everything else** goes to Avian 3D, exactly as before.

`Physics2d` is for the rare 2D physics entity with no visual of its own — an invisible trigger area, or the merged colliders a [tilemap layer generates](/docs/r1-alpha7/editor/tilemap). The 2D backend maps the shared shape data onto the plane: **Box** → rectangle (from `half_extents` X/Y), **Sphere** → circle, **Capsule** → 2D capsule, **Cylinder** → circle; a **Mesh** shape has no 2D source geometry and falls back to the fitted box. Of the axis locks, `lock_rotation_z` is the one that matters in 2D (it locks *the* rotation); use it on characters so collisions don't tip them over. Gravity, time scale and substeps from the [world physics settings](#world-physics-settings) mirror into the 2D world (2D gravity is the vector's X/Y).

## Physics components

You author physics with **two serializable, backend-agnostic components**. They save to scenes as plain data; the backend system reads them and spawns the real Avian components (`RigidBody`, `Collider`, `Mass`, `Friction`, …) when the entity becomes active.

- `PhysicsBodyData` — how the body moves (mass, damping, gravity, axis locks).
- `CollisionShapeData` — the collision geometry and surface properties.

An entity can have either or both. A `RuntimePhysics` marker is added once the backend components are wired up; changing the data at runtime re-applies them.

### Rigid body — `PhysicsBodyData`

| Body type (editor label) | Avian equivalent | Behavior |
|---|---|---|
| **Rigid Body** | `RigidBody::Dynamic` | Affected by gravity and forces. Has mass, falls, bounces, gets pushed. |
| **Static Body** | `RigidBody::Static` | Never moves. Floors, walls, immovable scenery. |
| **Kinematic Body** | `RigidBody::Kinematic` | Code-controlled. Pushes dynamic bodies but ignores forces. Moving platforms, characters. |

| Field | Default | Meaning |
|---|---|---|
| `mass` | `1.0` | Heavier bodies push lighter ones. |
| `gravity_scale` | `1.0` | `0` = floats, `1` = normal, `2` = double gravity. |
| `linear_damping` | `0.0` | Resists linear motion (drag). |
| `angular_damping` | `0.05` | Resists spin. |
| `lock_rotation_x/y/z` | `false` | Freeze rotation on an axis. |
| `lock_translation_x/y/z` | `false` | Freeze movement on an axis. |

### Collider — `CollisionShapeData`

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
| `friction` | `0.5` | `0.0` (ice) to high (grippy). |
| `restitution` | `0.0` | Bounciness: `0.0` (no bounce) to `1.0` (full bounce). |
| `is_sensor` | `false` | Pass-through trigger — detects overlap without blocking. |

> A **Mesh** collider is a concave trimesh and is best used on **static** geometry. For moving bodies prefer a primitive shape (box/sphere/capsule/cylinder).

## Adding physics in the editor

Select an entity, then in the **Inspector** use **Add Component**:

1. Add a **Physics Body** (inserts `PhysicsBodyData`). This also drops in a default **Collision Shape** automatically — auto-fitted to the entity's mesh bounds — unless the entity (or one of its children) already has one, so compound bodies built from child shapes are left alone.
2. Optionally adjust the **Collision Shape** (`CollisionShapeData`): pick a different shape from its dropdown, or add more shapes on child entities for a compound body.

The collision-shape card has an **Edit** toggle (collider edit mode) that hides the transform gizmo so you can resize/move the collider directly. In the **2D viewport** the same toggle shows a green collider frame with eight drag handles: drag a handle to resize (boxes resize from the grabbed edge; circle and capsule radii resize about the centre), drag inside the shape to move its offset — e.g. shrink a tree object's collider down to its trunk base. Each gesture is one undo step. While the toggle is on, clicks edit the collider instead of selecting/moving sprites; toggle it off to get the normal picker back.

## World physics settings

Global simulation settings live in `PhysicsPropertiesState`, edited from the editor's physics controls. They are world-level (not per-entity) and are **not** exposed to scripts.

| Setting | Default | Notes |
|---|---|---|
| Gravity preset | **Earth** | Earth `-9.81`, Moon `-1.62`, Mars `-3.72`, Jupiter `-24.79`, Zero-G `0`, or Custom (m/s² on Y). |
| Gravity vector | `0, -9.81, 0` | Set directly for arbitrary directions; switches the preset to Custom. |
| Time scale | `1.0` | Slows or speeds the whole simulation. |
| Substeps | `6` | Solver substeps per step (higher = more stable, more cost). |

## Scripting physics

Physics script helpers don't poke Avian directly — they push `ScriptAction`/`ScriptCommand` events that `renzora_physics` observes and applies to the **script's own entity**. The named helpers are sugar over those verbs.

### Forces and velocity

| Function | Lua | Rhai | Effect |
|---|---|---|---|
| `apply_force(x, y, z)` | yes | yes | Applies a force for the current frame (cleared every frame — call it each `on_update` for a sustained push). |
| `apply_impulse(x, y, z)` | yes | yes | Sets the body's linear velocity to this value (a simplified impulse in the current Avian backend). |
| `set_velocity(x, y, z)` | yes | yes | Directly sets the linear velocity. |
| `set_linear_velocity(x, y, z)` | yes | — | Lua alias for `set_velocity` (registered by the physics extension). |
| `set_gravity_scale(scale)` | yes | — | Adjusts this body's gravity scale at runtime. |
| `move_controller(x, y, z)` | yes | — | Kinematic collide-and-slide move (see below). |

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
end
```

```rhai
// thruster.rhai — apply_force / apply_impulse / set_velocity also work in Rhai
fn on_update() {
    set_velocity(input_x * 5.0, 0.0, input_y * 5.0);
}
```

> `apply_force`, `apply_impulse`, and `set_velocity` are available in **both** Lua and Rhai. `move_controller`, `set_linear_velocity`, and `set_gravity_scale` are **Lua-only** (the physics crate registers extra functions only on the Lua backend).

> **2D bodies:** `apply_force`, `apply_impulse`, and `set_velocity` route to the entity's own backend automatically — on an Avian 2D body they apply the X/Y of the vector and ignore Z, so the same script works on a sprite. `move_controller` is 3D-only for now; drive 2D characters with `set_velocity` (lock rotation on the body so collisions don't spin it).

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

> `action()` and `move_controller` are Lua-only. In Rhai, move a kinematic body with `set_velocity` / `set_position` / `translate` instead.

### Reading physics state

Per-entity physics is mirrored into a reflect-readable `PhysicsReadState` component, refreshed every frame. Read it with `get("PhysicsReadState.<field>")` — this works in **both** Lua and Rhai.

| Read | Type | Meaning |
|---|---|---|
| `get("PhysicsReadState.grounded")` | bool | A downward sweep found ground this frame (within `max_slope`). |
| `get("PhysicsReadState.velocity")` | vec3 | Linear velocity (world space). |
| `get("PhysicsReadState.speed")` | float | Magnitude of `velocity`. |
| `get("PhysicsReadState.ground_normal")` | vec3 | Contact normal of the last ground hit (`0,1,0` when airborne). |

For touch/overlap, the `is_colliding` context global is `true` whenever the entity has any active collision this frame (available in Lua and Rhai):

```rhai
fn on_update() {
    if is_colliding {
        print_log("touching something");
    }
}
```

> `grounded` and `ground_normal` are written by the kinematic slide, so they're meaningful for entities you drive with `move_controller`.

### What is *not* a script function

These appear in older docs or the internal `ScriptCommand` enum but have **no named text-script binding** — don't call them, they won't resolve:

- `apply_torque`, `set_angular_velocity` — rotational forces are not exposed to scripts.
- `raycast`, `raycast_down` — there is no raycast text function (raycasting exists only internally).
- `apply_impulse_to`, `find_entity_by_name` — no such helpers.
- `collisions_entered`, `collisions_exited`, `active_collisions` — these counters don't exist; use the `is_colliding` global.
- `on_collision` — there is **no collision lifecycle hook** in Lua or Rhai. Collision *events* (`on_collision_enter` / `on_collision_exit`) exist only as [Blueprint](/docs/r1-alpha5/scripting/blueprints) nodes.

## Lua vs Rhai summary

| Capability | Lua | Rhai |
|---|---|---|
| `apply_force` / `apply_impulse` / `set_velocity` | yes | yes |
| `set_linear_velocity` / `set_gravity_scale` | yes | — |
| `move_controller` / `kinematic_slide` action | yes | — |
| `get("PhysicsReadState.*")` reads | yes | yes |
| `is_colliding` global | yes | yes |

## Related

- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts attach and run
- [Rhai](/docs/r1-alpha5/scripting/rhai) — the cross-platform subset backend
- [Blueprints](/docs/r1-alpha5/scripting/blueprints) — collision and physics nodes for visual scripting
