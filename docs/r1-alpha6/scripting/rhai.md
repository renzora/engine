# Rhai

Write game logic in Rhai — a small, pure-Rust scripting language that runs everywhere, including WebAssembly builds.

## Rhai or Lua?

Renzora ships **two** script backends. The engine picks one by file extension: `.rhai` runs on the Rhai backend, `.lua` runs on the [Lua backend](/docs/r1-alpha5/scripting/lua). Both are compiled into the shipping runtime by default, and both produce the same underlying engine commands.

The practical difference is reach and surface area:

| | Rhai (`.rhai`) | Lua (`.lua`) |
|---|---|---|
| Implementation | `rhai` 1.21, pure Rust | `mlua` 0.10 (Lua 5.4), vendored C |
| Runs on web / WASM exports | **Yes** | No (native platforms only) |
| Lifecycle hooks | `props`, `on_ready`, `on_update` | + `on_rpc`, `on_ui`, `on_http`, `on_player_joined`, `on_player_left` |
| API surface | A subset (~58 functions) | The full surface (~70 functions + extensions) |
| Networking / HTTP / `action()` | Not available | Available |

> **Rule of thumb:** if your game targets the web, write gameplay that needs to run on every platform in Rhai. If you need networking, HTTP, the `action()` event bus, or the extra lifecycle hooks, use Lua (native-only). Rhai is a deliberate subset — see [Limitations](#what-rhai-cant-do) below.

## Your first script

Create `scripts/player.rhai`:

```rhai
fn on_ready() {
    print_log("Player spawned!");
}

fn on_update() {
    let speed = 5.0;
    // input_x / input_y are the WASD movement axes (read-only globals).
    // Call a transform function to actually move the entity.
    translate(input_x * speed * delta, 0.0, input_y * speed * delta);
}
```

Attach it to an entity in the **Inspector → Scripts** section. A `ScriptComponent` is added automatically to any entity that has a `Name`, so you just point it at the file. When you enter play mode, the entity moves with WASD.

> **Important:** the context inputs like `position_x`, `input_x`, and `delta` are read-only values pushed into the script scope each frame. Assigning to `position_x` does **not** move the entity — only declared script variables are read back. To change the world you must call a function such as `translate(...)` or `set_position(...)`.

## Lifecycle hooks

Rhai scripts implement up to three functions. Any that are missing are simply skipped.

| Hook | When it runs |
|---|---|
| `fn props()` | Once, to declare Inspector-editable variables. Returns an object map. |
| `fn on_ready()` | Once, when the script's entity becomes active. |
| `fn on_update()` | Every frame. |

```rhai
fn props() {
    #{ speed: 5.0 }
}

fn on_ready() {
    print_log("ready");
}

fn on_update() {
    // ...per-frame logic...
}
```

> The Lua-only hooks `on_rpc`, `on_ui`, `on_http`, `on_player_joined`, and `on_player_left` do **not** fire for Rhai scripts — the Rhai backend uses the engine's no-op defaults for them. There is no `on_start`, `on_collision`, or `on_destroy` hook in either language; use `on_ready` / `on_update` and the `is_colliding` global, or a [Blueprint](/docs/r1-alpha5/scripting/blueprints) collision node.

## Script properties

`props()` returns a Rhai object map (`#{ ... }`) of variables that show up as editable fields in the Inspector. Each entry can be a bare default value, or a map with `default`/`value`, an optional `hint` tooltip, and an optional `tab` to group fields.

```rhai
fn props() {
    #{
        // shorthand: bare default value
        speed: 5.0,

        // full form: default + tooltip + tab grouping
        jump_force: #{ default: 10.0, hint: "Upward impulse", tab: "Movement" },
        can_fly:    #{ default: false },
        tint:       #{ default: #{ r: 1.0, g: 0.2, b: 0.2, a: 1.0 } }
    }
}

fn on_update() {
    translate(input_x * speed * delta, 0.0, input_y * speed * delta);
}
```

The widget type is **inferred from the default value** — float, int, bool, string, vec2 (`#{ x, y }`), vec3 (`#{ x, y, z }`), or color (`#{ r, g, b, a }`). The values you declare become ordinary variables inside `on_ready`/`on_update`.

> The Rhai `props` parser reads only `value`/`default`, `hint`, and `tab`. Other keys such as `min`, `max`, or `type` are currently ignored — there is no slider-range metadata for Rhai props.

## Reading the world: context globals

Each frame the backend pushes the entity's state and input into the script scope as plain variables. Read them freely; treat them as read-only inputs.

| Global | Type | Meaning |
|---|---|---|
| `delta` | float | Seconds since last frame |
| `elapsed` | float | Seconds since start |
| `position_x` / `position_y` / `position_z` | float | World position |
| `rotation_x` / `rotation_y` / `rotation_z` | float | Euler rotation (degrees) |
| `scale_x` / `scale_y` / `scale_z` | float | Scale |
| `input_x` / `input_y` | float | WASD movement axes |
| `mouse_x` / `mouse_y` | float | Cursor position |
| `mouse_delta_x` / `mouse_delta_y` | float | Cursor movement this frame |
| `mouse_left` / `mouse_right` / `mouse_middle` | bool | Mouse buttons held |
| `mouse_left_just_pressed` / `mouse_right_just_pressed` | bool | Pressed this frame |
| `mouse_scroll` | float | Scroll delta |
| `camera_yaw` | float | Camera yaw (radians) |
| `camera_ev` | float | Scene EV-100 (auto-exposure readback) |
| `gamepad_left_x` / `gamepad_left_y` / `gamepad_right_x` / `gamepad_right_y` | float | Stick axes |
| `gamepad_left_trigger` / `gamepad_right_trigger` | float | Trigger pressure |
| `gamepad_south` / `east` / `west` / `north` / `l1` / `r1` / `l2` / `r2` / `l3` / `r3` / `select` / `start` | bool | Face/shoulder buttons |
| `gamepad_dpad_up` / `down` / `left` / `right` | bool | D-pad |
| `is_colliding` | bool | True if this entity has any active collisions |
| `timers_finished` | array | Names of timers that fired this frame |
| `self_entity_id` | int | This entity's id |
| `self_entity_name` | string | This entity's `Name` |
| `self_health` / `self_max_health` | float | Health values |
| `has_parent` | bool | Whether this entity has a parent |
| `parent_position_x` / `parent_position_y` / `parent_position_z` | float | Parent world position |

```rhai
fn on_update() {
    if is_colliding {
        print_log("touching something");
    }
    if mouse_left_just_pressed {
        print_log("clicked at " + mouse_x + ", " + mouse_y);
    }
}
```

## Function reference

These functions are registered on the Rhai backend. Coordinates and most numeric arguments are floats.

### Transform

| Function | Effect |
|---|---|
| `set_position(x, y, z)` | Set local position |
| `set_rotation(x, y, z)` | Set Euler rotation (degrees) |
| `set_scale(x, y, z)` / `set_scale_uniform(s)` | Set scale |
| `translate(x, y, z)` | Move relative to current position |
| `rotate(x, y, z)` | Rotate by Euler delta (degrees) |
| `look_at(x, y, z)` | Face a world point |
| `parent_set_position(x, y, z)` / `parent_set_rotation(x, y, z)` / `parent_translate(x, y, z)` | Drive the parent transform |
| `set_child_position(name, x, y, z)` / `set_child_rotation(name, x, y, z)` / `child_translate(name, x, y, z)` | Drive a named child transform |

### Physics, animation, audio

| Function | Effect |
|---|---|
| `apply_force(x, y, z)` / `apply_impulse(x, y, z)` / `set_velocity(x, y, z)` | Rigid-body forces |
| `play_animation(name)` / `stop_animation()` / `pause_animation()` / `resume_animation()` | Animation playback |
| `set_animation_speed(speed)` / `crossfade_animation(name, duration)` | Blend / speed |
| `set_anim_param(name, value)` / `set_anim_bool(name, value)` / `trigger_anim(name)` / `set_layer_weight(layer, weight)` | Animator state machine |
| `play_sound(path)` / `play_sound_at_volume(path, vol)` / `play_sound_looping(path, vol)` | One-shot SFX |
| `play_music(path)` / `stop_music()` / `stop_all_sounds()` | Music / bus control |

### Scene, environment, misc

| Function | Effect |
|---|---|
| `spawn_entity(name)` / `despawn_self()` | Create / remove entities |
| `set_visibility(visible)` | Show / hide |
| `set_sun_angles(azimuth, elevation)` / `set_fog(enabled, start, end)` | Environment |
| `screen_shake(intensity, duration)` | Camera shake |
| `lock_cursor()` / `unlock_cursor()` | Cursor grab |
| `start_timer(name, duration)` / `start_timer_repeat(name, duration)` / `stop_timer(name)` | Timers (fire into `timers_finished`) |
| `print_log(msg)` | Write to the engine console |
| `asset_progress()` / `is_loading()` / `is_loaded()` | Loading-screen state |
| `vec2(x, y)` / `vec3(x, y, z)` / `lerp(a, b, t)` / `clamp(v, min, max)` | Math helpers (return object maps / floats) |

In addition, Rhai's own standard library is available: arithmetic, strings, arrays, object maps, `if`/`while`/`for`, functions, and `print` / `debug`.

### Reflection: read and write components

`get` / `set` operate on the **self** entity by component path; `get_on` / `set_on` take an entity name first.

```rhai
fn on_update() {
    // read a mirrored subsystem field
    let grounded = get("PhysicsReadState.grounded");
    if grounded == true {
        // write a field on another entity
        set_on("Door", "DoorState.open", true);
    }
}
```

Paths are `"ComponentType.field"` (sub-fields with further dots). Reads of unknown paths return `()` (unit/empty).

### Keyboard input

Rhai exposes the live key tables as a special `_keys_pressed` / `_keys_just_pressed` / `_keys_just_released` map, which you pass to the query functions. Key names are Bevy `KeyCode` debug strings (`"KeyW"`, `"Space"`, `"ArrowUp"`, …):

```rhai
fn on_update() {
    if is_key_pressed(_keys_pressed, "KeyW") {
        translate(0.0, 0.0, 5.0 * delta);
    }
    if is_key_just_pressed(_keys_just_pressed, "Space") {
        apply_impulse(0.0, 8.0, 0.0);
    }
}
```

> For most movement, prefer the `input_x` / `input_y` axes and the `gamepad_*` globals — they already combine keyboard and gamepad. The action-mapped helpers (`input_button_pressed`, `input_axis_2d`, …) are **Lua-only**.

## Sharing values with the UI

Variables declared in `props()` (and any script variable you assign) are read back after every hook, so [Game UI](/docs/r1-alpha5/scripting/game-ui) markup can bind to them with `{{ Entity.var }}`:

```rhai
fn props() {
    #{ score: 0 }
}

fn on_update() {
    if is_colliding {
        score += 1;   // persisted and readable from markup as {{ self.score }}
    }
}
```

## Rhai language basics

Rhai reads like a blend of Rust and JavaScript:

```rhai
// Variables (dynamically typed)
let x = 42;
let name = "hello";

// Functions — the last expression is the return value
fn add(a, b) {
    a + b
}

// Control flow
if x > 10 {
    print_log("big");
} else {
    print_log("small");
}

// Loops
for i in 0..10 {
    print_log("" + i);
}

while x > 0 {
    x -= 1;
}

// Object maps (used for props and vec helpers)
let v = #{ x: 1.0, y: 2.0, z: 3.0 };
```

## What Rhai can't do

Rhai is a strict subset of the Lua surface. The following are **not** registered on the Rhai backend — reach for a `.lua` script (native platforms) if you need them:

- **The `action()` event bus** — `action()` / `action_on()`, and therefore every UI verb (`ui_*`, `hui_*`), `global_set`/`global_get`, navmesh actions, etc.
- **Networking** — `rpc()`, `net_is_server()`, `net_is_client()`, `net_is_connected()`, `net_player_count()`, and the `on_rpc` / `on_player_joined` / `on_player_left` hooks.
- **HTTP** — `http_get()`, `http_post()`, `json_parse()`, and the `on_http` hook.
- **Action-mapped input** — `input_button_pressed()` / `input_button_just_pressed()` / `input_button_just_released()`, `input_axis_1d()`, `input_axis_2d()`.
- **Component introspection** — `get_component()` / `get_component_on()`, `get_components()` / `get_components_on()`, `has_component()` / `has_component_on()` (use `get` / `set` instead).
- **Assorted helpers** — `spawn_primitive()`, `despawn_by_prefix()`, `load_scene()`, `set_material_color()`, `draw_line()`, `play_audio()`, `set_gravity_scale()`.

## Related

- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts attach and run
- [Lua](/docs/r1-alpha5/scripting/lua) — the full-surface, native-only backend
- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — node graphs interpreted at runtime
- [Scripting API Reference](/docs/r1-alpha5/api/scripting) — the complete function catalog
