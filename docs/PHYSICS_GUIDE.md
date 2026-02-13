# Physics Guide

A complete guide to the Renzora Engine physics system — rigid bodies, collision shapes, cloth simulation, debugging tools, stress testing, and scripting integration.

The engine uses **Avian 3D** as its physics backend, integrated with Bevy's ECS. Physics components are authored in the editor and activated at runtime when you enter play mode.

---

## Table of Contents

1. [How Physics Works in the Editor](#how-physics-works-in-the-editor)
2. [Rigid Bodies](#rigid-bodies)
3. [Collision Shapes](#collision-shapes)
4. [Cloth Simulation](#cloth-simulation)
5. [Global Physics Properties](#global-physics-properties)
6. [Physics Debug Panel](#physics-debug-panel)
7. [Physics Forces & Impulses Panel](#physics-forces--impulses-panel)
8. [Physics Metrics Panel](#physics-metrics-panel)
9. [Physics Playground (Stress Testing)](#physics-playground-stress-testing)
10. [Physics Scenarios (Preset Tests)](#physics-scenarios-preset-tests)
11. [Play Mode & Physics Lifecycle](#play-mode--physics-lifecycle)
12. [Scripting API for Physics](#scripting-api-for-physics)
13. [Setting Up Optimal Physics Tests](#setting-up-optimal-physics-tests)
14. [Best Practices](#best-practices)
15. [Troubleshooting](#troubleshooting)

---

## How Physics Works in the Editor

Physics in Renzora follows a **data-then-simulate** pattern:

1. **Design time**: You add `RigidBody` and `CollisionShape` components to entities in the editor. These are serializable data components that describe what the physics should be.
2. **Play mode**: When you press `F5` or `Shift+F5`, the engine converts your data components into live Avian physics components (forces, colliders, solvers). Physics begins simulating.
3. **Stop**: When you exit play mode, all runtime physics components are removed. Entities return to their original transforms. Nothing is permanently changed.

This means you can freely experiment — enter play mode, watch things fall, exit, and everything resets.

---

## Rigid Bodies

Add a **RigidBody** component to any entity to make it participate in physics simulation.

### Body Types

| Type | Behavior | Use Cases |
|------|----------|-----------|
| **Dynamic** | Affected by gravity, forces, and collisions. Has mass and inertia. | Crates, balls, ragdolls, projectiles, anything that moves |
| **Static** | Never moves. Infinite mass. Other objects collide with it. | Ground, walls, floors, fixed platforms, terrain |
| **Kinematic** | Moved only by code/scripts. Not affected by forces. Pushes dynamic objects. | Moving platforms, elevators, doors, scripted obstacles |

### Properties

| Property | Type | Range | Default | Description |
|----------|------|-------|---------|-------------|
| **Body Type** | Dropdown | Dynamic / Static / Kinematic | Dynamic | How the body behaves in simulation |
| **Mass** | Float | 0.001+ | 1.0 | Weight in kg. Higher mass = harder to move. Only for dynamic bodies. |
| **Gravity Scale** | Float | -10.0 to 10.0 | 1.0 | Multiplier on gravity. 0 = weightless, 0.5 = moon-like, -1 = reverse gravity |
| **Linear Damping** | Float | 0.0 to 10.0 | 0.0 | Drag on movement (air resistance). Higher = slower movement over time |
| **Angular Damping** | Float | 0.0 to 10.0 | 0.05 | Drag on rotation. Higher = rotation slows faster |
| **Lock Rotation X/Y/Z** | Bool | on/off | off | Prevent rotation on specific axes. Useful for keeping characters upright |
| **Lock Translation X/Y/Z** | Bool | on/off | off | Prevent movement on specific axes. Constrain to a plane or line |

### Property Notes

- **Mass** only appears for Dynamic bodies. Static and Kinematic bodies have infinite effective mass.
- **Gravity Scale** of 0 makes the object float in place (useful for space games or floating objects).
- **Linear Damping** of 0 means no air resistance — objects keep sliding forever on frictionless surfaces. A value of 1-3 gives a noticeable drag effect.
- **Angular Damping** at 0.05 (default) provides slight rotational slowdown. Set to 0 for perpetual spinning, or higher for objects that should stop rotating quickly.
- **Lock Rotation** is essential for character controllers — without it, your player capsule will tip over on collision.

---

## Collision Shapes

Add a **CollisionShape** component to define the physical shape used for collision detection. An entity needs both a RigidBody and a CollisionShape to participate in physics.

### Shape Types

| Shape | Parameters | Best For |
|-------|-----------|----------|
| **Box** | Half Extents (X, Y, Z) | Crates, walls, platforms, buildings |
| **Sphere** | Radius | Balls, projectiles, character detection zones |
| **Capsule** | Radius, Half Height | Character bodies, humanoid colliders, cylindrical shapes |
| **Cylinder** | Radius, Half Height | Barrels, pillars, wheels |

Only **one** collider type per entity. They conflict with each other — adding a Sphere removes an existing Box, etc.

### Shared Properties

| Property | Type | Range | Default | Description |
|----------|------|-------|---------|-------------|
| **Offset** | Vec3 | unlimited | (0, 0, 0) | Position offset from entity origin. Use when the visual mesh center doesn't match where you want the collider |
| **Friction** | Float | 0.0 to 1.0 | 0.5 | How much objects resist sliding. 0 = ice, 0.5 = normal, 1.0 = sticky rubber |
| **Restitution** | Float | 0.0 to 1.0 | 0.0 | Bounciness. 0 = no bounce (dead stop), 0.5 = moderate, 1.0 = perfect bounce |
| **Is Sensor** | Bool | on/off | off | When on, the collider detects overlaps but doesn't physically block anything |

### Shape Defaults

| Shape | Specific Defaults |
|-------|------------------|
| **Box** | Half extents: (0.5, 0.5, 0.5) — a 1x1x1 cube |
| **Sphere** | Radius: 0.5 |
| **Capsule** | Radius: 0.3, Half height: 0.5 |
| **Cylinder** | Radius: 0.3, Half height: 0.5 |

### Sensors

When **Is Sensor** is enabled:
- The collider still generates collision events (useful for scripts)
- Objects pass through it without being blocked
- Use for: trigger zones, damage areas, checkpoints, spawn regions, detection volumes

Scripts can detect sensor overlap via collision events (`on_collision_enter`, `on_collision_exit`).

### Friction Guide

| Value | Surface Feel | Example |
|-------|-------------|---------|
| 0.0 | Frictionless | Ice, oiled metal |
| 0.1 - 0.3 | Slippery | Polished wood, wet tile |
| 0.4 - 0.6 | Normal | Concrete, dry wood |
| 0.7 - 0.9 | Grippy | Rubber, rough stone |
| 1.0 | Maximum grip | Velcro-like |

### Restitution Guide

| Value | Bounce | Example |
|-------|--------|---------|
| 0.0 | None | Clay, sandbag |
| 0.1 - 0.3 | Slight | Wood block, heavy ball |
| 0.4 - 0.6 | Moderate | Tennis ball |
| 0.7 - 0.8 | High | Rubber ball |
| 0.9 - 1.0 | Near-perfect | Super ball (unrealistic above 0.95) |

---

## Cloth Simulation

The engine includes cloth physics via the **bevy_silk** crate with Avian collision integration.

### Adding Cloth

Add the **Cloth** component to an entity. The engine auto-generates a 20x20 vertex subdivided plane mesh with the top row pinned.

### Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| **Pinned Vertices** | Vertex list | Top row (0-19) | Vertices fixed to the entity transform. Controls where cloth is "attached" |
| **Stick Generation** | Quads / Triangles | Quads | How structural constraints are created. Triangles = more constraints, more rigid |
| **Stick Length** | Auto / Fixed / Offset / Coefficient | Auto | How constraint rest-lengths are computed |
| **Stick Mode** | Fixed / Spring | Fixed | Fixed = rigid sticks. Spring = stretchy with min/max percent |
| **Normal Computation** | None / Smooth / Flat | Smooth | How normals are recalculated each frame |

### Collision Settings

| Property | Range | Default | Description |
|----------|-------|---------|-------------|
| **Enable Collisions** | on/off | on | Whether cloth collides with other physics objects |
| **Collision Offset** | 0.0 to 10.0 | 0.25 | Distance buffer to prevent clipping through colliders |
| **Velocity Coefficient** | 0.0 to 5.0 | 1.0 | How much collider motion transfers to cloth. 0 = cloth ignores moving colliders |

### Per-Entity Overrides

These override global cloth settings for individual cloth entities:

| Override | Range | Default (if enabled) | Description |
|----------|-------|---------------------|-------------|
| **Custom Gravity** | (-100, -100, -100) to (100, 100, 100) | (0, -9.81, 0) | Override global gravity for this cloth |
| **Custom Friction** | 0.0 to 1.0 | 0.01 | Cloth surface friction |
| **Custom Sticks Depth** | 1 to 50 | 5 | Solver iterations. Higher = more rigid cloth, more computation |

### Cloth Tips

- **Hanging cloth**: Pin the top row of vertices
- **Flag**: Pin one edge (left or right column)
- **Canopy**: Pin all four corners
- **More vertices = better draping** but slower simulation
- **Increase sticks depth** for stiffer fabric (e.g., canvas vs silk)
- **Spring mode** with min 0.8 / max 1.2 gives natural stretch

---

## Global Physics Properties

Accessible from the physics properties panel. These affect the entire simulation.

### Gravity

| Preset | Value (m/s^2) | Feel |
|--------|--------------|------|
| **Earth** | (0, -9.81, 0) | Standard (default) |
| **Moon** | (0, -1.62, 0) | Floaty, slow falling |
| **Mars** | (0, -3.72, 0) | Between Moon and Earth |
| **Jupiter** | (0, -24.79, 0) | Heavy, fast falling |
| **Zero-G** | (0, 0, 0) | Space, no gravity |
| **Custom** | User-defined | Any direction and magnitude |

Gravity is a 3D vector — you can set it sideways for "falling sideways" effects or upward for reverse gravity.

### Time Scale

| Value | Effect |
|-------|--------|
| 0.1x | Extreme slow motion |
| 0.25x | Very slow |
| 0.5x | Half speed |
| 1.0x | Normal (default) |
| 2.0x | Double speed |
| 4.0x | Fast forward |

Range: 0.0 to 10.0. Affects all physics simulation speed.

### Solver Substeps

| Value | Accuracy | Performance | Use Case |
|-------|----------|-------------|----------|
| 1-2 | Low | Fast | Simple scenes, few collisions |
| 4-6 | Good (default: 6) | Moderate | General use |
| 8-12 | High | Slower | Tall stacks, precise constraints |
| 15-50 | Very high | Slow | Extreme precision, stress testing |

Higher substeps prevent objects from tunneling through each other and improve stacking stability. The default of 6 works well for most scenes.

---

## Physics Debug Panel

Shows real-time physics simulation state. Open from the debug panels menu.

### Sections

**Status Indicator**
- Shows Running or Paused with color feedback
- Physics runs during Play mode, pauses during Edit mode

**Rigid Bodies**
- Total count displayed prominently
- Breakdown by type with colored bar:
  - Blue = Dynamic
  - Yellow = Kinematic
  - Gray = Static

**Colliders**
- Total collider count
- Distribution by shape type (top 6 shown with bars)
- Shapes: Sphere, Box, Capsule, Cylinder, Cone, ConvexHull, TriMesh, Heightfield, Compound

**Step Time**
- Current physics computation time in milliseconds
- Average step time
- Color-coded performance feedback:
  - Green (2ms or less) = excellent
  - Yellow (2-5ms) = good
  - Orange (5-10ms) = acceptable
  - Red (10ms+) = slow, optimize scene
- History graph (120 samples)

**Collision Pairs**
- Number of active collision pairs
- Expandable list of first 10 pairs showing entity names and contact count

### Debug Visualization Toggles

| Toggle | What It Shows |
|--------|--------------|
| **Show Colliders** | Wireframe overlay of all collision shapes in the scene |
| **Show Contacts** | Points where objects are physically touching |
| **Show AABBs** | Axis-aligned bounding boxes (broad-phase collision volumes) |
| **Show Velocities** | Velocity vectors drawn as lines from each body |
| **Show Center of Mass** | Dot markers at each body's center of mass |
| **Show Joints** | Lines showing constraint/joint connections |

These toggles render gizmo overlays without recompiling. Very useful for diagnosing why collisions aren't working as expected.

---

## Physics Forces & Impulses Panel

Apply forces to selected entities for testing. Requires a selected entity with a RigidBody component.

### Force Modes

| Mode | Behavior | When to Use |
|------|----------|-------------|
| **Force** | Continuous acceleration (applied per frame, F=ma) | Wind, engines, continuous pushes |
| **Impulse** | Instant velocity change (applied once) | Explosions, jumps, bullet impacts |
| **Torque** | Rotational force around entity center | Spinning objects, rotation effects |
| **Velocity Override** | Set velocity directly, ignoring physics | Teleport with momentum, animation sync |

### Direction Presets

Quick buttons for common directions: Up, Down, Left, Right, Forward, Back, or Custom vector input.

### Magnitude

- Range: 0.1 to 1000.0 (logarithmic slider)
- Default: 10.0
- Scales the force/impulse vector

### Explosion Tool

Applies a radial impulse to all dynamic bodies within a radius:

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| **Radius** | 0.1 to 100m | 10.0 | Blast radius |
| **Force** | 1 to 1000 | 20.0 | Impulse magnitude at center |

Force falls off linearly from center to edge. Objects at the edge receive minimal force.

### Current Velocity Display

Shows the selected entity's:
- Linear velocity (v) in m/s
- Angular velocity (omega) in rad/s

---

## Physics Metrics Panel

Real-time energy and performance tracking for the physics simulation.

### Energy Tracking

| Metric | Formula | What It Tells You |
|--------|---------|-------------------|
| **Kinetic Energy** | 0.5 x mass x velocity^2 | Total motion energy in the system |
| **Potential Energy** | mass x gravity x height | Stored energy from elevation |
| **Total Energy** | KE + PE | Should decrease over time with friction. If it grows, something is unstable |

Each metric includes a sparkline history graph.

### Body Metrics

| Metric | Description |
|--------|-------------|
| **Total Bodies** | Sum of active + sleeping dynamic bodies |
| **Active Bodies** | Bodies currently moving (green indicator) |
| **Sleeping Bodies** | Bodies at rest — not being simulated (gray indicator, good for performance) |

### Velocity & Momentum

| Metric | Description |
|--------|-------------|
| **Average Speed** | Mean velocity magnitude across all dynamic bodies |
| **Max Speed** | Highest velocity in the scene |
| **Momentum** | Vector sum of mass x velocity. Should be roughly conserved in closed systems |

### Physics Time

Estimated computation time per frame in milliseconds, with a history sparkline. Helps identify when physics is becoming a bottleneck.

---

## Physics Playground (Stress Testing)

A tool for spawning batches of physics objects to test performance and behavior.

### Shapes

Sphere, Box, Capsule, Cylinder — select which shape to spawn.

### Spawn Patterns

| Pattern | Description | What It Tests |
|---------|-------------|---------------|
| **Single** | One object at spawn height | Basic setup verification |
| **Stack** | Vertical tower | Stacking stability, solver accuracy |
| **Wall** | Grid formation | Lateral collisions, structural integrity |
| **Rain** | Circular spread falling from above | Mass collision handling, performance |
| **Pyramid** | 3D pyramid shape | Stacking under load |
| **Explosion** | Radial distribution with outward velocity | Rapid collision resolution |

### Parameters

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| **Count** | 1-500 | 10 | Number of objects per spawn |
| **Mass** | 0.01-1000 | 1.0 | Mass of each object |
| **Restitution** | 0.0-1.0 | 0.3 | Bounciness |
| **Friction** | 0.0-2.0 | 0.5 | Surface friction |
| **Spawn Height** | 0-100m | 10.0 | Y position |

### Usage

1. Select shape and pattern
2. Adjust parameters
3. Click **Spawn** to create the batch
4. Enter play mode (`F5`) to see simulation
5. Click **Clear All** to remove playground entities

Playground entities are tagged for easy cleanup and won't pollute your actual scene when cleared.

---

## Physics Scenarios (Preset Tests)

Pre-built physics demonstrations that test specific behaviors. Great for verifying your physics settings or learning how properties interact.

### Available Scenarios

| # | Scenario | Description | Tests |
|---|----------|-------------|-------|
| 1 | **Newton's Cradle** | 5 hanging spheres, classic momentum demo | Joint constraints, momentum transfer |
| 2 | **Domino Chain** | 20 dominoes in a curved line | Cascade collisions, friction |
| 3 | **Wrecking Ball** | Heavy ball on joint vs. wall of boxes | Heavy impacts, joint constraints |
| 4 | **Stack Test** | Tower of 15 boxes | Solver stability, stacking accuracy |
| 5 | **Billiard Break** | 15 balls in triangle + cue ball | Sphere collisions, momentum conservation |
| 6 | **Inclined Plane** | Ramp with sphere and box | Friction behavior, gravity on slopes |
| 7 | **Pendulum** | Sphere on distance joint | Simple harmonic motion, energy conservation |
| 8 | **Projectile Launch** | Ball launched at 45 degrees toward wall | Ballistic trajectory, gravity |
| 9 | **Gauntlet** | Obstacle course with ramps and swinging objects | Complex multi-body interaction |
| 10 | **Avalanche** | 30 objects on steep slope | Large-scale collisions, performance |
| 11 | **Wedge Stress** | V-trough, corner, and funnel geometry | Tight geometry stability, edge cases |

### Scale Multiplier

- Range: 0.5x to 3.0x
- Scales all dimensions, masses, and velocities proportionally
- Use smaller scale for desktop testing, larger for dramatic effect

### How to Use

1. Open the Physics Scenarios panel
2. Select a scenario
3. Optionally adjust the scale multiplier
4. Click **Spawn Scenario**
5. Enter play mode (`F5`) to watch the simulation
6. Click **Clear** to remove scenario entities

---

## Play Mode & Physics Lifecycle

### What Happens When You Press F5

| Step | Action |
|------|--------|
| 1 | Engine finds all entities with `PhysicsBodyData` or `CollisionShapeData` |
| 2 | Converts data components to live Avian components: `RigidBody`, `Mass`, `GravityScale`, `LinearDamping`, `AngularDamping`, `LockedAxes`, `Collider`, `Friction`, `Restitution`, `Sensor` |
| 3 | Marks entities with `RuntimePhysics` for cleanup tracking |
| 4 | Unpauses physics time |
| 5 | Simulation runs each frame |

### What Happens When You Stop

| Step | Action |
|------|--------|
| 1 | All `RuntimePhysics`-marked components are removed |
| 2 | Physics time is paused |
| 3 | Entities return to their editor-time state |

### Play Mode States and Physics

| State | Physics | How to Enter |
|-------|---------|-------------|
| **Editing** | Paused | Default state / press Escape during play |
| **Playing** | Running | Press F5 |
| **Paused** | Paused | Press F6 during play |
| **Scripts Only** | Running | Press Shift+F5 |
| **Scripts Paused** | Paused | Pause during scripts-only mode |

**Scripts Only** mode (`Shift+F5`) is useful for testing physics while keeping the editor camera and UI visible.

---

## Scripting API for Physics

Use these functions in `.rhai` scripts to interact with physics at runtime.

### Applying Forces

```rhai
// Continuous force (applied every frame - good for engines, wind)
apply_force(0.0, 0.0, -50.0);        // Push forward
apply_force_to(other_entity, 10.0, 0.0, 0.0);  // Push another entity

// Instant impulse (applied once - good for jumps, explosions)
apply_impulse(0.0, 10.0, 0.0);       // Jump upward

// Direct velocity control
set_velocity(5.0, 0.0, 0.0);         // Move right at 5 m/s
```

### Raycasting

```rhai
// Cast a ray downward from position to check for ground
let hit = raycast(pos.x, pos.y, pos.z, 0.0, -1.0, 0.0, 10.0, "ground_check");
```

Parameters: origin (x,y,z), direction (x,y,z), max distance, result variable name.

### Collision Events

Scripts can respond to collision events:
- `on_collision_enter` — first frame of contact
- `on_collision_exit` — contact ended
- Works with sensors for trigger zones

### Example: Simple Character Controller

```rhai
fn props() {
    return #{
        speed: #{ value: 5.0 },
        jump_force: #{ value: 8.0 },
        is_grounded: #{ value: false }
    };
}

fn on_update() {
    let speed = get_prop("speed");
    let jump_force = get_prop("jump_force");

    // Movement
    let h = get_axis_horizontal();
    let v = get_axis_vertical();
    apply_force(h * speed, 0.0, v * speed);

    // Jump
    if is_key_just_pressed("Space") {
        apply_impulse(0.0, jump_force, 0.0);
    }
}
```

---

## Setting Up Optimal Physics Tests

### Basic Physics Test Scene

1. **Create a ground plane**:
   - Add a Cube, scale it to (20, 0.5, 20)
   - Add RigidBody (set to **Static**)
   - Add CollisionShape Box with half extents (10, 0.25, 10)

2. **Create a test object**:
   - Add a Sphere
   - Add RigidBody (keep as **Dynamic**, mass 1.0)
   - Add CollisionShape Sphere, radius 0.5
   - Position it above the ground (Y = 5)

3. **Press F5** and watch it fall and bounce

### Testing Stacking Stability

1. Create ground as above
2. Spawn a **Stack** pattern from the Physics Playground (10 boxes)
3. Open **Global Physics Properties**:
   - Set substeps to 6 (default) for baseline
   - Set friction to 0.7 for grip
4. Press F5 and observe
5. If the stack collapses, increase substeps to 10-12
6. If performance drops, reduce substeps

### Testing Collision Response

1. Create ground plane
2. Create two spheres side by side
3. Set one with **restitution 0.0** and the other with **restitution 0.9**
4. Drop both from the same height
5. Compare bounce behavior

### Testing Friction on Slopes

1. Use the **Inclined Plane** scenario, or:
2. Create a cube, rotate it 30 degrees on Z, set as Static with Box collider
3. Place a Dynamic box on top with various friction values
4. Low friction (0.1) = slides down, high friction (0.8) = stays put

### Performance Stress Test

1. Open Physics Playground
2. Select **Rain** pattern, count = 200
3. Press Spawn, then F5
4. Watch the Physics Debug panel:
   - Step time should stay under 5ms (green/yellow)
   - If it goes red (10ms+), reduce count or simplify colliders
5. Try again with 500 to find your scene's limit

### Testing Sensors/Triggers

1. Create a large Box collider, enable **Is Sensor**
2. Attach a script that prints when collision occurs:
   ```rhai
   fn on_update() {
       // Check collision events
       if has_collision() {
           debug_log("Something entered the trigger zone!");
       }
   }
   ```
3. Drop a Dynamic object through it — it should pass through while triggering the event

---

## Best Practices

### General Rules

- **Always use Static bodies for non-moving geometry** (floors, walls, buildings). Dynamic bodies that never move waste computation.
- **Avoid extreme mass ratios**. A 0.01kg ball hitting a 10000kg wall is fine (static walls have infinite mass). But a 0.01kg box stacked on a 10000kg box can be unstable. Keep mass ratios under 100:1 between interacting dynamic bodies.
- **Match collider shape to visual mesh** as closely as possible. A sphere collider on a cube will roll when it shouldn't.
- **Use capsules for characters**. They slide smoothly over terrain and don't catch on edges like boxes do.
- **Lock rotation axes for characters** to prevent tipping over.

### Performance

- **Use simple collider shapes**. Box and Sphere are cheapest. Mesh colliders are expensive — avoid them when a primitive will do.
- **Sleeping is free**. Avian automatically sleeps bodies that stop moving. Don't wake them unnecessarily.
- **Reduce substeps** if you have many objects and don't need perfect stacking. 4 substeps is fine for most action games.
- **Use sensors instead of raycasts** when possible. A sensor volume is cheaper than per-frame raycasts.
- **Monitor step time** in the Physics Debug panel. Under 5ms is healthy. Over 10ms means you should optimize.

### Realism

| Scenario | Recommended Settings |
|----------|---------------------|
| Realistic indoor scene | Gravity: Earth, Friction: 0.5-0.7, Restitution: 0.1-0.3, Substeps: 6 |
| Space game | Gravity: Zero-G, Linear damping: 0.5-2.0 (for thruster feel) |
| Platformer | Gravity: Earth or 1.5x, Gravity scale per-character for floaty jumps |
| Racing game | Friction: 0.6-0.9, Low restitution, High linear damping for braking |
| Ragdoll | All rotation locks off, Low angular damping, Moderate mass |
| Puzzle game | High substeps (10+), Precise friction, Moderate restitution |

### Common Mistakes

| Mistake | Fix |
|---------|-----|
| Objects fall through floor | Ensure floor has both RigidBody (Static) AND CollisionShape |
| Stack collapses immediately | Increase solver substeps to 8-12, increase friction to 0.6+ |
| Objects vibrate/jitter | Reduce restitution, increase damping, check for overlapping colliders |
| Character tips over | Lock rotation X and Z axes |
| Objects explode apart on spawn | They're overlapping. Space them out or reduce restitution |
| Physics feels floaty | Increase gravity scale or use higher gravity preset |
| Physics feels too fast | Reduce time scale to 0.5-0.75 |
| Sensor not detecting | Ensure the other object also has a collider (both need colliders for events) |

---

## Troubleshooting

### "Physics not running"

- Are you in play mode? Physics only simulates during play (F5 or Shift+F5).
- Check the Physics Debug panel — does it show "Running"?
- Is the `physics` cargo feature enabled? (It is by default.)

### "Object falls through floor"

- The floor needs BOTH a RigidBody (Static) and a CollisionShape.
- Check that the collider size matches the visual mesh. A tiny collider on a large floor will have gaps.
- At very high velocities, objects can tunnel through thin colliders. Use thicker colliders or increase substeps.

### "Collision shapes don't match visuals"

- Enable **Show Colliders** in the Physics Debug panel to see wireframe outlines.
- Adjust collider half-extents, radius, or offset to match the mesh.
- Remember: half-extents of (0.5, 0.5, 0.5) creates a 1x1x1 box.

### "Physics is slow"

- Check step time in the Physics Debug panel.
- Reduce the number of dynamic bodies. Convert stationary objects to Static.
- Use simpler collider shapes (Box/Sphere instead of Mesh).
- Reduce solver substeps (try 4 instead of 6).
- Check for "sleeping" bodies in the Metrics panel. If none are sleeping, objects may be vibrating and preventing sleep.

### "Cloth clips through objects"

- Increase the collision offset (default 0.25, try 0.5+).
- Increase sticks depth for more rigid cloth.
- Ensure the collider the cloth is interacting with is large enough.
