# Parkour & Traversal

Vaulting a rail, mantling onto a roof, hanging off a ledge and shimmying along it, climbing a ladder, running along a wall and kicking off it, swinging from a rope — provided by the `renzora_parkour` plugin.

Almost none of it needs authoring. Ledges, walls and their heights are found by casting rays at whatever collision geometry is already in the scene, so ordinary level meshes are vaultable and climbable the moment a character with a `Parkour Controller` runs at them. Only two things need a marker, because no cast can infer them: a **ladder** (a ladder and a fence are the same shape) and a **swing anchor** (a point in space with nothing to touch).

## The controller owns movement

This is the part to read before wiring anything up.

The engine has no built-in character controller: a kinematic body moves because a script calls `move_controller()`, which is collide-and-slide and nothing else. `Parkour Controller` replaces that entirely — it runs gravity, ground contact, jumping, collision response **and** the traversals.

It has to. Hanging off a lip, or being pinned to a sphere around a rope anchor, or being halfway through an arc over a fence, are *positions*, not forces; no velocity fed to collide-and-slide lands on them exactly, and "nearly right" in a traversal system reads as the character clipping through the ledge they were supposed to catch.

So a parkour character is driven with `parkour_move()` instead of `move_controller()`. Calling both on the same entity means two systems writing the same `Transform` each frame, and whichever ran last wins.

## Setting up a character

Add **Parkour Controller** to the entity that should move — usually the same entity that has the animator.

- **No physics component is required.** The controller sweeps a capsule built from its own `radius` / `height` and writes `Transform` directly, so it needs neither a body nor a collider to move, fall, or traverse.
- **Add a Collision Shape if other things need to hit the character** — bullets, triggers, enemies. A collider on its own is the safest pairing: the physics engine reads its transform to keep it in place and never writes back, so there is nothing to fight over.
- **If you add a Physics Body it must be a Kinematic Body.** The default type is Rigid Body, which is *dynamic*: the solver applies gravity, integrates a position and writes that back over the controller every frame. The two then fight, and the character sinks, drifts or jitters. Kinematic is written to but never moved on its own, so it round-trips harmlessly — and it lets the character push dynamic objects around, which a bare collider cannot.
- Set `foot_offset` if the model's origin isn't at its feet. `0` means origin at the soles, which is how most imported characters sit; a model with its origin at the hips wants roughly half its height.

The character's own colliders are excluded from every probe and sweep, **including any on its children** — an imported model normally keeps its collider on a child mesh rather than on the entity the controller sits on, and a capsule that collides with its own body cannot move at all.

Then drive it from a script:

```lua
-- player.lua
function on_update()
    local x, y = input_axis_2d("move")

    -- World-relative: -Z is forward. `input_axis_2d` returns +Y for "up" on
    -- the stick, so it is negated to point down -Z.
    parkour_move(x, 0, -y)

    parkour_sprint(input_button_pressed("sprint"))

    if input_button_just_pressed("jump") then parkour_jump() end
    if input_button_just_pressed("interact") then parkour_action() end
    if input_button_just_pressed("crouch") then parkour_release() end
end
```

For a third-person game you want this rotated into camera space before it is
passed in — the controller takes a world direction and does not know where the
camera is looking. Rotating by the camera's yaw `t` gives:

```lua
local sin, cos = math.sin(t), math.cos(t)
parkour_move(-sin * -y + cos * x, 0, -cos * -y - sin * x)
```

`parkour_move()` is **consumed each frame**, not latched — the same contract as `move_controller()`. Call it every frame you want the character to move; stop calling it and they stop.

> **Only in Play or Simulate.** Every parkour system is gated on the simulation running, so a character sits exactly where you placed them while editing. Use **Play**, or **[Simulate](/docs/r1-alpha7/editor/viewport#simulate-mode)** to watch them move with the editor still live.

## The states

The controller is in exactly one state at a time, readable as `get("ParkourReadState.state")`:

| State | What it is | How you leave it |
|---|---|---|
| `grounded` | Standing or running on walkable ground | Jump, walk off an edge, or start a traversal |
| `airborne` | Falling or rising under gravity | Land, grab a ledge, start a wall run, grab a rope |
| `vaulting` | Playing a vault over a rail | Finishes on its own, into `airborne` |
| `mantling` | Climbing up onto a ledge | Finishes on its own, into `airborne` above the lip |
| `hanging` | Hanging from a lip by the hands | Climb up, drop, or shimmy sideways along it |
| `climbing` | On a ladder | Top out, step off at the bottom, jump off, or let go |
| `wall_running` | Running along a wall, on the clock | Time out, run out of wall, or wall-jump |
| `swinging` | On a rope, as a pendulum | Let go, jump off, or swing into something |

### Vault vs mantle

Both start from the same probe; what separates them is what the ground does on the **far side** of the lip.

- The ground drops away within `vault_max_depth` → it is a **rail**, and the character **vaults** over it and lands beyond, keeping their momentum.
- The ground is level with the top → it is a **platform**, and the character **mantles** onto it and stands there.

Height decides whether either is possible at all: up to `vault_max_height` (1.2 m) can be vaulted, up to `mantle_max_height` (2.3 m) can be mantled, and anything below `step_height` (0.4 m) is just walked over without a traversal — otherwise the character would vault every kerb.

Traversals are authored motion. Once one starts, the character follows a fixed curve with gravity and collision switched off for its duration, because the probe that started it already proved the whole path was clear — re-testing it every frame can only make the character collide with the very obstacle they are deliberately passing over.

With `auto_traverse` on (the default) vaults and mantles trigger on contact. Turn it off and they need `parkour_action()`, which is what a game with a "press E to climb" prompt wants — `ParkourReadState.can_vault` / `can_mantle` tell you when to show it.

### Ledge hanging

While airborne, a ledge between 55% and 115% of the character's height catches them (`ledge_grab`, on by default). From a hang:

- Left/right input **shimmies** along the lip — and only onto lip that is really there, so the character can't slide off the end of a balcony still in the hang pose.
- `parkour_jump()`, `parkour_action()`, or up input **climbs up** (a mantle).
- `parkour_release()` or down input **drops off**.

### Ladders

Add **Parkour Ladder** to the ladder object — or to any ancestor of its collider, since the lookup walks up the hierarchy, so a collider buried inside an imported model still counts.

| Field | Default | Effect |
|---|---|---|
| `climb_speed_scale` | `1.0` | Multiplies the controller's `climb_speed`, so a rope ladder can be slower than a steel one without retuning the character |
| `auto_attach` | `true` | Latch on by walking into it. Off requires `parkour_action()` |
| `exit_at_top` | `true` | Mantle onto the top when the climb runs out of ladder. Off leaves the character on the top rung until they jump or let go — right for a ladder up a wall, wrong for one into a hatch |

While climbing, `y` in `parkour_move()` is the climb axis and the horizontal part is ignored: a ladder is a rail, and letting the stick push the character sideways off it mid-climb is the most common way ladder controllers feel broken.

### Wall running and wall jumps

Both come from `wall_run`. A wall run needs a near-vertical surface within about a third of a metre of the character, a horizontal speed above 80% of `walk_speed`, and movement input; it lasts `wall_run_duration` seconds under the much weaker `wall_run_gravity`, which is what makes it read as running rather than sliding. Starting one sheds most of the jump’s climb, so the run holds its height instead of carrying the character metres up the wall.

**One run per wall, per ground touch.** Once a wall has been run, that same wall will not catch the character again until they land — otherwise the run simply restarts the moment it expires, and `wall_run_duration` limits nothing. A wall facing a different way is a fresh opportunity straight away, so a corridor can still be zig-zagged up.

A wall jump works off any wall in reach — beside *or* in front — so running into a wall and pressing jump kicks off it. It takes priority over every other use of the button while airborne, because that is the one the player pressed *at* a wall.

### Swinging

Add **Parkour Swing Anchor** to an empty entity at the pivot point — a rope's fixing, a bar, a hook, a vine. It needs no collider; anchors are found by proximity and a line-of-sight check.

| Field | Default | Effect |
|---|---|---|
| `rope_length` | `0.0` | Rope length in metres. `0` uses however far away the character was when they grabbed it — right for a vine, wrong for a trapeze bar |
| `max_grab_distance` | `6.0` | How far away it can be grabbed from |
| `damping` | `0.35` | Energy lost per second, as a fraction of speed. `0` swings forever |

Grab with `parkour_action()` while airborne. Movement input pumps the swing along its travel; it is deliberately weak, because a swing the player can drive like a walk stops reading as a rope. `parkour_release()` lets go; `parkour_jump()` lets go with `swing_release_boost` extra height on top — often the difference between clearing the gap and not.

Swinging through a wall is prevented by sweeping each step: hitting something lets go rather than clipping.

### Blocking geometry

Ray-probed traversal is occasionally right about geometry a designer wanted left alone — the lip of a bottomless pit, a decorative railing that should read as impassable, a collision proxy standing in for something soft. Add **Parkour Blocker** to it (or to an ancestor) and every probe ignores it.

## Tuning

`Parkour Controller` defaults describe an adult human: 1.8 m tall, runs at 7 m/s, jumps 1.15 m, vaults a 1.2 m rail, mantles a 2.3 m wall.

| Field | Default | Effect |
|---|---|---|
| `radius` / `height` | `0.35` / `1.8` | The swept capsule. Ledge-grab reach is derived from `height`, so a shorter character grabs lower ledges |
| `foot_offset` | `0.0` | Distance from the entity origin *down* to the soles |
| `walk_speed` / `run_speed` | `4.0` / `7.0` | Ground speed, with and without `parkour_sprint(true)` |
| `acceleration` | `40.0` | How fast the target speed is reached, m/s² |
| `air_control` | `0.35` | Fraction of `acceleration` that still applies mid-air |
| `gravity` | `-22.0` | Downward acceleration. Jump *height* is independent of it — the launch speed is derived — so this can be tuned for feel without changing how high the character jumps |
| `terminal_velocity` | `-55.0` | Fastest the character may fall |
| `jump_height` | `1.15` | Apex of a standing jump, in metres |
| `max_slope` | `55.0` | Steepest surface that counts as ground, in degrees |
| `step_height` | `0.4` | Walked over without a traversal; also how far the controller snaps up **or** down to follow stairs and slopes without a traversal |
| `coyote_time` | `0.12` | Grace period after walking off an edge during which jump still works |
| `jump_buffer_time` | `0.15` | How long a jump or action press is remembered |
| `face_movement` / `turn_speed` | `true` / `12.0` | Turn the character toward where they are going |
| `facing_offset` | `0.0` | Degrees to turn the **model** by, on top of the direction the character is facing. Purely cosmetic. Bevy treats `-Z` as forward, but glTF characters — anything out of Mixamo especially — are usually authored facing `+Z`, and imported as-is they travel correctly while appearing to run backwards. Set `180` and they face the way they are going |
| `auto_traverse` | `true` | Vault and mantle on contact, without `parkour_action()` |
| `forward_reach` | `0.55` | How far ahead obstacles are looked for |
| `vault_max_height` / `vault_max_depth` / `vault_duration` | `1.2` / `1.1` / `0.45` | Vault limits and how long the move plays |
| `mantle_max_height` / `mantle_duration` | `2.3` / `0.7` | Mantle limits and how long the move plays |
| `ledge_grab` / `hang_shimmy_speed` | `true` / `1.3` | Catch ledges when airborne; sideways speed while hanging |
| `climb_speed` | `2.2` | Ladder speed, scaled per-ladder |
| `wall_run` | `true` | Enables wall running *and* wall jumping |
| `wall_run_speed` / `wall_run_duration` / `wall_run_gravity` | `6.5` / `1.5` / `-3.0` | How fast, how long, and how much it sags |
| `wall_jump_up` / `wall_jump_away` | `6.0` / `5.0` | Wall-jump speed, up and outward |
| `swing` / `swing_release_boost` | `true` / `2.0` | Enables swinging; extra upward speed on letting go |

## Seeing what it is doing

Turn on **Gizmos → Physics** in the viewport and a parkour character draws its own diagnostics. It shares that dropdown with collider wireframes, including the *Selected only* and *Always* settings.

The important thing it shows is the **swept capsule** — built from `radius`, `height` and `foot_offset`, and *not* the collision shape you authored. That capsule is what decides where the character can go, and nothing else in the viewport shows it, so a capsule that does not match its model looks exactly like one that does.

It is drawn upright and unscaled, because that is how the controller casts it. If your character is leaning but the capsule is not, something else is rotating the entity.

| Drawn | Means |
|---|---|
| Capsule, green | Grounded |
| Capsule, amber | Airborne — if it never turns green, `foot_offset` is probably wrong |
| Capsule, violet | Playing a vault or mantle (gravity and collision off) |
| Capsule, blue | Holding a ledge, ladder or rope |
| Capsule, pink | Wall running |
| Cross at the soles, and a line up to the origin | Where the controller thinks the feet are. The line appears only when `foot_offset` is non-zero |
| Disc + normal at the feet | Ground contact and its surface normal |
| Grey line ahead at knee height | The direction and reach everything is probed along |
| Cross at a lip, with a drop line to foot height | A ledge, and its height above the feet — the number the whole decision turns on |
| Lip green / amber / blue / grey | It would vault it / mantle it / could grab it in mid-air / found it but will not act |
| Second green cross beyond a lip | Where a vault would land |
| Pink line from the chest | A wall in reach, at the distance sensed |
| Violet arc | The path a traversal is following, and where it ends |

Everything after the capsule comes from what the controller recorded on its last frame, not from fresh casts — so it shows what the state machine actually decided from, and it only appears while the simulation is running.

### Collider colours

Collider wireframes are coloured by body type: **green** static, **violet** kinematic, **orange** dynamic, **blue** sensor. Violet versus orange is worth knowing here — a character left on the default Rigid Body (dynamic) looks fine until the solver starts fighting the controller for its transform, and the colour is the fastest way to spot it.

## Animation

Add **Parkour Animations** and the controller crossfades one clip per state for you. The defaults are the names an imported character most often already has (`idle`, `walk`, `run`, `jump`, `fall`, `vault`, `mantle`, `hang`, `shimmy`, `climb`, `wall_run`, `swing`), plus a `blend` duration.

An empty name means "don't drive this state", so a project can let the controller handle locomotion and keep the traversals for itself. Leave the component off entirely and the controller drives no animation at all — read `ParkourReadState.state` and run your own [state machine](/docs/r1-alpha7/editor/animation) instead.

`jump` plays while *rising* and `fall` while descending, so a character who walked off a ledge falls rather than playing a jump they never made.

## Scripting

| Function | Effect |
|---|---|
| `parkour_move(x, y, z)` | Movement intent in world space. `x`/`z` steer, `y` climbs on ladders and hangs. Call every frame — it is consumed, not latched |
| `parkour_sprint(on)` | Move at `run_speed` instead of `walk_speed` |
| `parkour_jump()` | Jump, wall-jump, climb up from a hang, or let go of a swing with a boost |
| `parkour_action()` | Context traversal: vault, mantle, grab a ledge, mount a ladder, grab a rope |
| `parkour_release()` | Let go of whatever is being held — ledge, ladder or swing |

Reads go through reflection on `ParkourReadState`, like every other component:

| Field | Type | Meaning |
|---|---|---|
| `state` | string | One of the state names in the table above |
| `event` | string | The event that fired **this frame**, else empty (see below) |
| `grounded` | bool | Standing on walkable ground |
| `velocity` / `speed` | Vec3 / float | Current velocity and its magnitude |
| `traversing` | bool | A vault or mantle is playing — stop feeding input and hold the camera |
| `hanging`, `climbing`, `wall_running`, `swinging` | bool | The individual states, for convenience |
| `can_vault`, `can_mantle`, `can_grab` | bool | There is an obstacle in front that could be traversed right now — what to hang a button prompt on |
| `near_ladder` | bool | Facing a `Parkour Ladder` |
| `ledge_height` | float | Height of the ledge ahead above the feet, `0` if none |

`event` is set for exactly one frame, so polling it once per `on_update` catches each event once. The values are `jump`, `land`, `vault_start`, `vault_end`, `mantle_start`, `mantle_end`, `grab`, `release`, `ladder_mount`, `ladder_dismount`, `wall_run_start`, `wall_run_end`, `wall_jump`, `swing_grab`, `swing_release`.

```lua
function on_update()
    -- Don't fight the controller while it is playing a move.
    if get("ParkourReadState.traversing") then return end

    local ev = get("ParkourReadState.event")
    if ev == "land" then play_sound("footstep_land")
    elseif ev == "grab" then play_sound("ledge_grab") end

    if get("ParkourReadState.can_vault") then
        ui_show("VaultPrompt")
    else
        ui_hide("VaultPrompt")
    end
end
```

## See also

- [Physics](/docs/r1-alpha7/scripting/physics) — bodies, colliders, and the `move_controller` this replaces
- [Ragdoll Physics](/docs/r1-alpha7/scripting/ragdoll) — handing the same skeleton over to the solver
- [Animation](/docs/r1-alpha7/editor/animation) — clips, state machines and event markers
- [Scripting API](/docs/r1-alpha7/api/scripting) — the full function list
