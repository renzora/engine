# Scripting API — Complete Reference Guide

## Overview

The engine uses **Rhai** as its scripting language — a lightweight, sandboxed scripting language designed for embedding in Rust. Scripts are stored as `.rhai` files (and optionally `.blueprint` files in the editor, which compile to Rhai).

Scripts attach to entities via a `ScriptComponent` and execute two lifecycle hooks each frame:
- **`on_ready()`** — called once when the script first runs (or after hot-reload)
- **`on_update()`** — called every frame

---

## 1. Script Structure

Every script is a plain `.rhai` file. The minimal template:

```js
fn on_ready() {
    log("Hello from on_ready!");
}

fn on_update() {
    // runs every frame
}
```

### Props (Editor Inspector Variables)

You can expose variables to the editor inspector with a `props()` function:

```js
fn props() {
    #{
        speed: #{ default: 5.0, min: 0.0, max: 100.0 },
        jump_force: #{ default: 10.0 },
        label: #{ default: "Player" }
    }
}

fn on_update() {
    // 'speed', 'jump_force', 'label' are in scope as variables
    translate(input_x * speed * delta, 0.0, input_y * speed * delta);
}
```

Props defined this way become editable in the entity's inspector panel.

---

## 2. Scope Variables (Built-in Globals)

These variables are automatically available in every script, updated each frame before `on_update()` runs.

### Time

| Variable | Type | Description |
|---|---|---|
| `delta` | `f64` | Frame delta time in seconds |
| `elapsed` | `f64` | Total elapsed time since start |

### Self Transform

| Variable | Type | Description |
|---|---|---|
| `position_x`, `position_y`, `position_z` | `f64` | Current world position |
| `rotation_x`, `rotation_y`, `rotation_z` | `f64` | Current rotation (degrees) |
| `scale_x`, `scale_y`, `scale_z` | `f64` | Current scale |

### Self Identity

| Variable | Type | Description |
|---|---|---|
| `self_entity_id` | `i64` | This entity's raw ID |
| `self_entity_name` | `String` | This entity's name |

### Input — Keyboard

| Variable | Type | Description |
|---|---|---|
| `input_x`, `input_y` | `f64` | Movement axis (-1 to 1, from WASD/arrows) |
| `_keys_pressed` | `Map` | All currently held keys |
| `_keys_just_pressed` | `Map` | Keys pressed this frame |

### Input — Mouse

| Variable | Type | Description |
|---|---|---|
| `mouse_x`, `mouse_y` | `f64` | Mouse position (screen coords) |
| `mouse_delta_x`, `mouse_delta_y` | `f64` | Mouse movement this frame |
| `mouse_button_left`, `mouse_button_right`, `mouse_button_middle` | `bool` | Mouse button state |

### Input — Gamepad

| Variable | Type | Description |
|---|---|---|
| `gamepad_left_x`, `gamepad_left_y` | `f64` | Left stick |
| `gamepad_right_x`, `gamepad_right_y` | `f64` | Right stick |
| `gamepad_south`, `gamepad_east`, `gamepad_north`, `gamepad_west` | `bool` | Face buttons |
| `gamepad_left_trigger`, `gamepad_right_trigger` | `f64` | Triggers (0.0–1.0) |
| `gamepad_left_shoulder`, `gamepad_right_shoulder` | `bool` | Bumpers |

### Collision Data

| Variable | Type | Description |
|---|---|---|
| `collisions_entered` | `Array` | Entity IDs of new collisions this frame |
| `collisions_exited` | `Array` | Entity IDs of collisions ending this frame |
| `active_collisions` | `Array` | All currently colliding entity IDs |

### Timer Data

| Variable | Type | Description |
|---|---|---|
| `timers_finished` | `Map` | Timers that fired this frame |
| `timers_active` | `Map` | All active timers |

### Component Data

| Variable | Type | Description |
|---|---|---|
| `self_health` | `Map` | `{current, max, regen, invincible}` |
| `self_light` | `Map` | `{intensity, color_r, color_g, color_b, light_type}` |
| `self_material` | `Map` | `{color_r, color_g, color_b, color_a}` |

### Hierarchy

| Variable | Type | Description |
|---|---|---|
| `parent_entity_id` | `i64` | Parent entity ID (-1 if none) |
| `parent_position_x/y/z` | `f64` | Parent transform |
| `parent_rotation_x/y/z` | `f64` | Parent rotation |
| `children_count` | `i64` | Number of children |
| `_children_data` | `Map` | child name -> `{entity_id, position, rotation, scale}` |

### Lookup Tables

| Variable | Type | Description |
|---|---|---|
| `_found_entities` | `Map` | name -> entity_id mapping |
| `_entities_by_tag` | `Map` | tag -> array of entity_ids |
| `_raycast_results` | `Map` | results keyed by variable name |

---

## 3. Entity Management (ECS)

### Spawning

```js
spawn_entity("MyEntity");           // empty entity
spawn_cube("Box1");                  // primitive shapes
spawn_sphere("Ball1");
spawn_plane("Ground");
spawn_cylinder("Pillar");
spawn_capsule("Capsule1");

// Spawn at a specific position
spawn_cube_at("Box2", 5.0, 0.0, 3.0);
spawn_sphere_at("Ball2", 0.0, 10.0, 0.0);
spawn_plane_at("Floor2", 0.0, -1.0, 0.0);
spawn_cylinder_at("Pillar2", -3.0, 0.0, 0.0);
spawn_capsule_at("Cap2", 1.0, 2.0, 3.0);
```

### Despawning

```js
despawn_self();                      // destroy this entity
despawn_entity(some_entity_id);      // destroy by ID
```

### Finding Entities

```js
// By name (returns i64 entity ID, or -1 if not found)
let enemy_id = find_entity_by_name(_found_entities, "Enemy");
let exists = entity_exists(_found_entities, "Enemy");      // bool

// By tag
let enemies = get_entities_by_tag(_entities_by_tag, "enemy");  // Array of IDs
let any = has_entities_with_tag(_entities_by_tag, "enemy");    // bool
let n = count_entities_by_tag(_entities_by_tag, "enemy");      // i64
```

### Naming & Tagging

```js
set_entity_name(some_id, "NewName");

// Tags on self
add_tag("player");
remove_tag("player");

// Tags on other entities
add_tag_to(enemy_id, "marked");
remove_tag_from(enemy_id, "marked");
```

---

## 4. The Entity Access System (`entity()` / `get()` / `set()`)

This is the **primary way to read and write properties on any entity**, including other entities.

### Getting an entity handle

```js
let e = entity("EnemyName");     // look up by name -> returns a Map
let e2 = entity_id(some_raw_id); // look up by raw i64 ID
let p = parent();                // parent entity
let c = child("ChildName");     // child by name
let all_kids = children();       // Array of all children as Maps
```

### Reading properties with `get()`

```js
let e = entity("Enemy");
let x = get(e, "position_x");
let name = get(e, "name");
let vis = get(e, "visible");
let tag = get(e, "tag");
// Works for: position_x/y/z, rotation_x/y/z, scale_x/y/z, name, tag, visible
// Also reads user script variables on that entity
```

### Writing properties with `set()`

```js
let e = entity("Enemy");
set(e, "position_x", 10.0);
set(e, "visible", false);
set(e, "scale_x", 2.0);
```

> **IMPORTANT — Deferred writes**: `set()` calls are **not immediate**. They are queued into
> `DeferredPropertyWrites` and applied by an exclusive system. This means **cross-entity writes
> take effect next frame**, not the current one. If you `set()` a property then immediately
> `get()` it, you'll see the old value.

---

## 5. Transform

### Self Transform

```js
// Absolute positioning
set_position(10.0, 0.0, 5.0);
set_rotation(0.0, 90.0, 0.0);     // degrees
set_scale(2.0, 2.0, 2.0);
set_scale_uniform(1.5);

// Relative movement
translate(1.0 * delta, 0.0, 0.0);  // move right
rotate(0.0, 45.0 * delta, 0.0);    // spin

// Direction
look_at(target_x, target_y, target_z);
```

### Parent Transform

```js
parent_set_position(0.0, 5.0, 0.0);
parent_set_rotation(0.0, 0.0, 0.0);
parent_translate(1.0, 0.0, 0.0);
```

### Child Transform

```js
set_child_position("Turret", 0.0, 1.0, 0.0);
set_child_rotation("Turret", 0.0, 45.0, 0.0);
child_translate("Turret", 0.0, 0.0, 1.0 * delta);
```

### Vector Helpers

```js
let v = vec3(1.0, 2.0, 3.0);   // #{x: 1.0, y: 2.0, z: 3.0}
let v2 = vec2(1.0, 2.0);       // #{x: 1.0, y: 2.0}
```

---

## 6. Physics

The physics backend is **Avian3D**. Entities need a physics body component to respond to forces.

### Forces & Impulses

```js
// Self
apply_force(0.0, 100.0, 0.0);     // continuous force (use in on_update)
apply_impulse(0.0, 50.0, 0.0);    // instant kick (use for jumps, hits)
apply_torque(0.0, 10.0, 0.0);     // rotational force

// Other entity
apply_force_to(enemy_id, 0.0, -50.0, 0.0);
apply_impulse_to(enemy_id, 10.0, 0.0, 0.0);
```

### Velocity

```js
set_velocity(0.0, 0.0, 5.0);
set_velocity_of(enemy_id, 0.0, 0.0, 0.0);  // freeze entity
set_angular_velocity(0.0, 3.14, 0.0);
set_gravity_scale(0.0);  // disable gravity (0 = no gravity, 1 = normal)
```

### Raycasting

```js
// Cast a ray from origin in direction, up to max_dist
// Results stored in _raycast_results["my_ray"]
raycast(
    position_x, position_y, position_z,    // origin
    0.0, 0.0, -1.0,                         // direction
    100.0,                                    // max distance
    "my_ray"                                  // result variable name
);

// Shorthand for downward ray (useful for ground detection)
raycast_down(position_x, position_y, position_z, 5.0, "ground_check");

// Reading results (available next frame or later in the same frame depending on timing)
// Results contain: hit entity ID, hit point, hit normal, distance
```

---

## 7. Input Handling

### Keyboard

```js
// Check specific keys
if is_key_pressed(_keys_pressed, "Space") {
    apply_impulse(0.0, jump_force, 0.0);
}

if is_key_just_pressed(_keys_just_pressed, "E") {
    log("Interact!");
}

if is_key_just_released(_keys_pressed, "Shift") {
    log("Stopped sprinting");
}

// Movement axis (pre-computed from WASD/arrows)
let move_x = input_x;  // -1 (left/A) to 1 (right/D)
let move_z = input_y;  // -1 (back/S) to 1 (forward/W)
translate(move_x * speed * delta, 0.0, move_z * speed * delta);
```

### Mouse

```js
let pos = get_mouse_position(mouse_x, mouse_y);     // #{x, y}
let d = get_mouse_delta(mouse_delta_x, mouse_delta_y); // #{x, y}

if mouse_button_left {
    log("Shooting!");
}
```

### Gamepad

```js
let left = get_left_stick(gamepad_left_x, gamepad_left_y);
let right = get_right_stick(gamepad_right_x, gamepad_right_y);
let axes = get_gamepad_axis(gamepad_left_x, gamepad_left_y, gamepad_right_x, gamepad_right_y);
let movement = get_movement_axis(input_x, input_y);

if gamepad_south {  // A button (Xbox) / Cross (PlayStation)
    apply_impulse(0.0, jump_force, 0.0);
}
```

---

## 8. Timers & Delays

```js
fn on_ready() {
    start_timer("spawn_wave", 5.0);          // fires once after 5s
    start_timer_repeating("tick", 1.0);       // fires every 1s
}

fn on_update() {
    if timer_just_finished(timers_finished, "spawn_wave") {
        spawn_cube_at("Enemy", 0.0, 5.0, 0.0);
    }

    if timer_just_finished(timers_finished, "tick") {
        log("Tick! " + format_time(elapsed));
    }

    // Timer inspection
    let p = timer_progress(timers_active, "spawn_wave");   // 0.0 to 1.0
    let r = timer_remaining(timers_active, "spawn_wave");  // seconds left

    // Timer control
    pause_timer("tick");
    resume_timer("tick");
    stop_timer("tick");  // removes it entirely
}
```

### Quick delay alias

```js
delay(3.0, "my_delay");  // equivalent to start_timer("my_delay", 3.0)
```

### Time formatting

```js
let s = format_time(elapsed);          // "02:35"
let s2 = format_time_precise(elapsed); // "02:35.42"
```

---

## 9. Health System

Entities with a HealthComponent expose a health API:

```js
// Self
set_health(50.0);
set_max_health(200.0);
damage(25.0);
heal(10.0);
set_invincible(true);
set_invincible_duration(true, 2.0);  // 2 seconds of invincibility
kill();
revive();  // restores to max

// Check death
if is_dead(self_health) {
    log("I'm dead! Health: " + self_health["current"]);
}

// Reading health values from the scope map
let hp = self_health["current"];
let max = self_health["max"];

// Other entities
set_health_of(enemy_id, 0.0);
damage_entity(enemy_id, 50.0);
kill_entity(enemy_id);
```

---

## 10. Rendering & Materials

### Visibility

```js
set_visible(true);
set_visible_of(enemy_id, false);
show();   // shorthand for set_visible(true)
hide();   // shorthand for set_visible(false)
```

### Material Colors

```js
set_color(1.0, 0.0, 0.0, 1.0);     // RGBA (red, fully opaque)
set_color_rgb(0.0, 1.0, 0.0);       // RGB (green, alpha=1)
set_color_of(enemy_id, 1.0, 0.0, 0.0, 0.5);  // semi-transparent red
set_opacity(0.5);                     // just change alpha
```

### Lights

```js
set_light_intensity(500.0);
set_light_color(1.0, 0.9, 0.8);
set_light_intensity_of(lamp_id, 1000.0);
set_light_color_of(lamp_id, 0.0, 0.0, 1.0);  // blue light
```

### Sprites

```js
set_sprite_color(1.0, 1.0, 1.0, 1.0);  // tint
```

---

## 11. Audio

```js
// One-shot sounds
play_sound("sounds/explosion.ogg");
play_sound_at_volume("sounds/hit.ogg", 0.5);

// Looping sounds
play_sound_looping("sounds/engine.ogg", 0.8);

// 3D spatial audio
play_sound_3d("sounds/steps.ogg", position_x, position_y, position_z);
play_sound_3d_at_volume("sounds/steps.ogg", 0.7, position_x, position_y, position_z);

// Music
play_music("music/theme.ogg");
play_music_with_fade("music/battle.ogg", 0.8, 2.0);  // volume, fade-in seconds
stop_music();
stop_music_with_fade(1.5);  // fade-out seconds

// Global
set_master_volume(0.5);
stop_all_sounds();
```

---

## 12. Animation & Tweening

### Skeletal/Clip Animation

```js
play_animation("Run");                       // loop
play_animation_once("Jump");                 // play once
play_animation_speed("Walk", 0.5);           // half speed
play_animation_on(npc_id, "Idle");           // on other entity

stop_animation();
pause_animation();
resume_animation();
set_animation_speed(2.0);
```

### Sprite Animation

```js
play_sprite_animation("walk_cycle");
play_sprite_animation_once("attack");
set_sprite_frame(3);
```

### Tweening

Tweens smoothly interpolate a property over time. Available easing functions: any standard easing name (e.g. `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"`, etc.)

```js
tween_to("position_x", 10.0, 2.0, "ease_in_out");           // property, target, duration, easing
tween_position(10.0, 5.0, 0.0, 1.5, "ease_out");            // x,y,z, duration, easing
tween_rotation(0.0, 180.0, 0.0, 1.0, "ease_in_out");
tween_scale(2.0, 2.0, 2.0, 0.5, "ease_out");
```

---

## 13. Camera

```js
// Direct control
set_camera_position(0.0, 10.0, -5.0);
set_camera_target(0.0, 0.0, 0.0);
camera_look_at(position_x, position_y, position_z);
set_camera_zoom(2.0);

// Follow system
camera_follow(self_entity_id);
camera_follow_self();                                       // shorthand
camera_follow_with_offset(self_entity_id, 0.0, 10.0, -8.0);
camera_follow_with_params(self_entity_id, 0.0, 10.0, -8.0, 5.0);  // smoothing factor
camera_stop_follow();

// Effects
screen_shake(0.5, 0.3);   // intensity, duration
screen_shake_once(0.3);    // quick shake

// Coordinate conversion
world_to_screen(position_x, position_y, position_z);
screen_to_ray(mouse_x, mouse_y);
```

---

## 14. Environment

```js
// Sun
set_sun_angles(180.0, 45.0);           // azimuth, elevation
set_sun_direction(0.0, -1.0, -0.5);

// Ambient
set_ambient_brightness(0.3);
set_ambient_color(0.2, 0.2, 0.3);

// Sky
set_sky_top_color(0.1, 0.1, 0.4);
set_sky_horizon_color(0.8, 0.5, 0.3);

// Fog
set_fog(true, 10.0, 100.0);            // enabled, start, end
enable_fog(10.0, 100.0);
disable_fog();
set_fog_color(0.5, 0.5, 0.5);

// Exposure
set_ev100(10.0);
set_exposure(10.0);                     // alias
```

---

## 15. Scene & Prefabs

```js
load_scene("scenes/level2.scn");
spawn_prefab("prefabs/enemy.glb", 5.0, 0.0, 3.0);
spawn_prefab_rotated("prefabs/tree.glb", 0.0, 0.0, 0.0, 0.0, 45.0, 0.0);
spawn_prefab_here("prefabs/bullet.glb");   // spawn at self position

// Using Map positions
let pos = vec3(1.0, 2.0, 3.0);
let rot = vec3(0.0, 90.0, 0.0);
spawn_prefab_at("prefabs/item.glb", pos);
spawn_prefab_at_transform("prefabs/item.glb", pos, rot);
```

---

## 16. Particles

```js
let emitter_id = find_entity_by_name(_found_entities, "FireParticles");

particle_play(emitter_id);
particle_pause(emitter_id);
particle_stop(emitter_id);
particle_reset(emitter_id);

particle_burst(emitter_id, 50);              // emit 50 particles instantly
particle_set_rate(emitter_id, 2.0);          // 2x emission rate
particle_set_scale(emitter_id, 0.5);         // half-size particles
particle_set_time_scale(emitter_id, 1.5);    // faster playback

// Tinting
particle_set_tint(emitter_id, 1.0, 0.0, 0.0, 1.0);    // red
particle_set_tint_rgb(emitter_id, 0.0, 1.0, 0.0);      // green (alpha=1)
particle_clear_tint(emitter_id);

// Variables
particle_set_variable_float(emitter_id, "wind_speed", 5.0);
particle_set_variable_color(emitter_id, "base_color", 1.0, 0.5, 0.0, 1.0);
particle_set_variable_vec3(emitter_id, "direction", 0.0, 1.0, 0.0);

// Positional emission
particle_emit_at(emitter_id, 5.0, 0.0, 3.0);
particle_emit_at_with_count(emitter_id, 5.0, 0.0, 3.0, 20);
```

---

## 17. Generic Component Access

For components that don't have dedicated API functions, you can set fields generically:

```js
set_component_float("MyComponent", "speed", 10.0);
set_component_int("MyComponent", "count", 5);
set_component_bool("MyComponent", "active", true);
set_component_string("MyComponent", "label", "Hello");
```

---

## 18. Math Library

### Constants

```js
let p = pi();    // 3.14159...
let t = tau();   // 6.28318...
let e_val = e(); // 2.71828...
```

### Trigonometry

```js
sin(x); cos(x); tan(x);
asin(x); acos(x); atan(x); atan2(y, x);
```

### Common Operations

```js
sqrt(x); abs(x); floor(x); ceil(x); round(x);
pow(base, exp); exp(x); ln(x); log(x, base); log10(x); log2(x);
sign(x); step(edge, x); trunc(x); fract(x);
```

### Clamping & Min/Max

```js
min_f(a, b); max_f(a, b); clamp_f(value, min, max);   // floats
min_i(a, b); max_i(a, b); clamp_i(value, min, max);   // integers
```

### Interpolation

```js
lerp(a, b, t);               // linear interpolation
inverse_lerp(a, b, value);   // find t for value between a and b
smoothstep(a, b, t);         // smooth hermite interpolation
smootherstep(a, b, t);       // smoother (Ken Perlin's version)
move_towards(current, target, max_delta);  // clamped step
```

### Angles (Degrees)

```js
deg_to_rad(180.0);          // -> pi
rad_to_deg(pi());           // -> 180
normalize_angle(370.0);     // -> 10.0
angle_difference(350.0, 10.0);  // -> 20.0 (shortest path)
lerp_angle(0.0, 350.0, 0.5);   // -> 355.0 (wraps correctly)
```

### Vectors

```js
distance(x1,y1,z1, x2,y2,z2);
distance_2d(x1,y1, x2,y2);
length(x,y,z);
length_2d(x,y);
dot(x1,y1,z1, x2,y2,z2);
dot_2d(x1,y1, x2,y2);
```

### Random

```js
random();                    // 0.0 to 1.0
random_range(5.0, 10.0);    // float in range
random_int(1, 6);            // integer in range (inclusive)
```

---

## 19. Debug Tools

```js
// Logging
log("General message");
log_info("Info");
log_warn("Warning!");
log_error("Error!");
log_debug("Debug detail");

// Assertions
assert(health > 0, "Health should be positive");

// Visual debug drawing
draw_line(0.0,0.0,0.0, 5.0,5.0,5.0);                        // white line
draw_line_color(0.0,0.0,0.0, 5.0,5.0,5.0, 1.0,0.0,0.0);    // red line
draw_line_duration(0.0,0.0,0.0, 5.0,5.0,5.0, 0.0,1.0,0.0, 2.0);  // green, 2s

draw_ray(position_x, position_y, position_z, 0.0, 0.0, -1.0, 10.0);
draw_sphere(0.0, 5.0, 0.0, 1.0);
draw_sphere_color(0.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0);     // yellow
draw_box(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
draw_box_color(0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 1.0);  // blue
draw_point(5.0, 0.0, 0.0, 0.5);
```

---

## 20. Execution Lifecycle & Frame Order

```
PreScript          -> Entity data store populated (transforms, names, tags, etc.)
  |
ScriptExecution    -> For each entity with ScriptComponent:
  |                    1. If not initialized -> call on_ready(), mark initialized
  |                    2. Call on_update()
  |                    3. Read back modified scope vars (position, rotation, scale)
  |                    4. Drain command buffer -> route to typed queues
  |
CommandProcessing  -> Separate systems process physics, audio, rendering, camera queues
  |
DebugDraw          -> Debug draw commands rendered
  |
Cleanup            -> Temporary state cleared
```

---

## 21. Hot Reload

When a `.rhai` file is modified on disk:
1. The file modification time is detected
2. The AST is recompiled
3. `initialized` is reset to `false`
4. `on_ready()` is called again next frame

**All runtime scope variables are reset.** Persistent state must be stored in `ScriptVariables` (the `props()` values) to survive hot-reload.

---

## 22. Editor vs Runtime

There are two parallel scripting systems. Scripts written for the editor may use APIs unavailable in the standalone runtime.

| Aspect | Editor (`src/scripting/`) | Runtime (`src/runtime/scripting/`) |
|---|---|---|
| API functions | ~200+ across 16 modules | ~80 across 8 modules |
| Blueprint support | Yes | No |
| Props / inspector | Yes | No |
| Particles API | Yes | No |
| Scene loading API | Yes | No |
| Tweens / animation | Yes | No |
| Generic component access | Yes | No |

There is no compile-time check to warn when a script uses editor-only APIs.

---

## 23. Gotchas & Limitations

1. **Cross-entity `set()` is deferred one frame.** If you write a property on another entity, the change won't be visible until next frame.

2. **Entity IDs are opaque `i64` values.** They are raw `u64` bits cast to `i64` via `Entity::to_bits()`. Don't do arithmetic on them.

3. **`on_fixed_update()` and `on_destroy()` are NOT wired up.** Only `on_ready()` and `on_update()` are called by the Rhai engine.

4. **No async/await.** All execution is synchronous within a single frame. Use timers + state variables for multi-frame operations.

5. **Hot-reload resets all state.** Only `props()` variables survive. Design accordingly.

6. **Random uses a simple LCG.** Fine for gameplay, not for anything requiring quality randomness.

7. **Mid-frame entity data can be stale.** The entity data store is built once at frame start and cached. If script A spawns an entity, script B (running later the same frame) won't see it in `_found_entities`.

8. **Multiple scripts per entity are supported.** Each `ScriptEntry` has its own scope and variables. They execute in order.

9. **Blueprints are editor-only.** `.blueprint` files are compiled to Rhai before execution and are not supported by the runtime system.

---

## 24. Complete Example: Player Controller

```js
fn props() {
    #{
        speed: #{ default: 8.0, min: 0.0, max: 50.0 },
        jump_force: #{ default: 12.0 },
        health_max: #{ default: 100.0 },
        dash_cooldown: #{ default: 1.0 }
    }
}

fn on_ready() {
    set_max_health(health_max);
    set_health(health_max);
    camera_follow_with_params(self_entity_id, 0.0, 12.0, -10.0, 5.0);
    add_tag("player");
    start_timer_repeating("regen_tick", 2.0);
    log("Player ready: " + self_entity_name);
}

fn on_update() {
    // --- Movement ---
    let move_x = input_x * speed;
    let move_z = input_y * speed;
    translate(move_x * delta, 0.0, move_z * delta);

    // Face movement direction
    if abs(move_x) > 0.1 || abs(move_z) > 0.1 {
        let angle = rad_to_deg(atan2(move_x, move_z));
        set_rotation(0.0, angle, 0.0);
    }

    // --- Jump (ground check) ---
    raycast_down(position_x, position_y, position_z, 1.2, "ground");
    if is_key_just_pressed(_keys_just_pressed, "Space") {
        apply_impulse(0.0, jump_force, 0.0);
    }

    // --- Dash ---
    if is_key_just_pressed(_keys_just_pressed, "ShiftLeft") {
        if !timer_progress(timers_active, "dash_cd") > 0.0 {
            apply_impulse(input_x * 20.0, 0.0, input_y * 20.0);
            start_timer("dash_cd", dash_cooldown);
            screen_shake_once(0.2);
        }
    }

    // --- Collision handling ---
    for id in collisions_entered {
        let other = entity_id(id);
        let tag = get(other, "tag");
        if tag == "enemy" {
            damage(10.0);
            set_invincible_duration(true, 0.5);
            screen_shake(0.3, 0.2);
            play_sound("sounds/hit.ogg");
        }
        if tag == "pickup" {
            heal(25.0);
            despawn_entity(id);
            play_sound("sounds/pickup.ogg");
        }
    }

    // --- Health regen ---
    if timer_just_finished(timers_finished, "regen_tick") {
        if !is_dead(self_health) {
            heal(1.0);
        }
    }

    // --- Death ---
    if is_dead(self_health) {
        log("Player died!");
        set_color(0.3, 0.3, 0.3, 0.5);
        // Could load a game over scene
    }

    // --- Debug visualization ---
    draw_sphere_color(position_x, position_y + 2.0, position_z, 0.3, 0.0, 1.0, 0.0);
}
```
