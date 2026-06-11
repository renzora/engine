# Scripting API

The authoritative reference for every global, function, lifecycle hook, and `action()` verb exposed to Lua and Rhai scripts.

This page documents the surface registered by `crates/renzora_scripting` (`backends/lua.rs`, `backends/rhai.rs`, `command.rs`) plus the functions injected by domain crates. For a guided introduction see [Lua](/docs/r1-alpha5/scripting/lua), [Rhai](/docs/r1-alpha5/scripting/rhai), and the [Scripting Overview](/docs/r1-alpha5/scripting/overview).

## How the API is dispatched

The scripting core is language-agnostic. A `ScriptEngine` resource holds a list of `ScriptBackend`s and routes each script to one **by file extension**:

| Extension | Backend | Availability |
|-----------|---------|--------------|
| `.lua` | Lua (mlua 0.10, Lua 5.4, vendored) | Native desktop + mobile only — **not** on the web (wasm) build |
| `.rhai` | Rhai (rhai 1.21, pure Rust) | Everywhere, **including wasm** |

Both backends are compiled into the shipping runtime (`renzora_scripting` default features = `["lua", "rhai"]`). Every callable in this reference is one of two things:

- A **registered function** — a Rust closure exposed to the VM that pushes a `ScriptCommand` onto a per-frame queue, applied after the hook returns.
- A **context global** — a value written fresh into the VM before each hook. Globals are inputs; assigning to them has no effect.

A `ScriptComponent` is auto-inserted on any entity that receives a `Name`. One Lua VM is cached per `(entity, script_path)` and reused across frames.

> Rhai is a deliberate **subset** of the Lua surface. Each function below is tagged **Both** (registered in Lua and Rhai) or **Lua** (Lua only). Rhai also supports only three lifecycle hooks. See [Rhai subset](#rhai-subset) at the end.

## Lifecycle hooks

Define any of these free functions; the engine calls the ones that exist. None are required.

| Hook | Fires when | Backends |
|------|-----------|----------|
| `props()` | Once on load — returns a table of editable Inspector properties | Both |
| `on_ready()` | Once, the first frame the script runs | Both |
| `on_update()` | Every frame | Both |
| `on_rpc(name, args, from)` | A networked RPC arrived. `from` is the sender's peer id (`0` for relayed messages) | Lua |
| `on_ui(name, args, entity)` | A markup UI event fired. `entity` is the firing node's `Entity::to_bits()` as a **u64 integer**, not a handle | Lua |
| `on_http(callback, status, body)` | An HTTP response returned. `status` is the HTTP code; **`status == 0` means the request failed** and `body` holds the error text | Lua |
| `on_player_joined(id)` | A player connected (**server/host only**) | Lua |
| `on_player_left(id)` | A player disconnected (server/host only) | Lua |

> There is **no** `on_start`, `on_collision`, or `on_destroy` hook. Use `on_ready` for setup and read the `is_colliding` global for overlap state. Collision *events* exist only as [Blueprint](/docs/r1-alpha5/scripting/blueprints) nodes. Rhai scripts get `props`, `on_ready`, and `on_update` only — the other hooks fall through to no-ops.

### props()

`props()` returns a table of variables that appear as editable fields in the Inspector:

```lua
function props()
    return {
        speed     = { value = 5.0, hint = "Movement speed in m/s" },
        jump      = { default = 10.0, tab = "Movement" },
        can_fly   = false,
        team_name = "Red",
    }
end
```

- Each entry is a bare value, or a table with `value` (or `default`), optional `hint`, and optional `tab`.
- The widget type is **inferred from the value** (`ScriptValue`: Float, Int, Bool, String, Entity, Vec2, Vec3, Color). A `type` key is ignored, and `min`/`max` are not read.
- Declared properties become read/write globals inside every hook. After each hook the engine reads them back, so changes persist and can bind into UI with `{{ Entity.speed }}`.

In Rhai use a map literal: `#{ speed: #{ value: 5.0, hint: "..." } }`.

## Context globals

Written fresh before each hook. Read them — do not assign.

### Time, transform, entity

| Global | Type | Description |
|--------|------|-------------|
| `delta` | number | Seconds since the previous frame |
| `elapsed` | number | Seconds since startup |
| `position_x` / `_y` / `_z` | number | World position |
| `rotation_x` / `_y` / `_z` | number | Euler rotation, **degrees** |
| `scale_x` / `_y` / `_z` | number | World scale |
| `self_entity_id` | integer | This entity's id (bits) |
| `self_entity_name` | string | This entity's `Name` |
| `self_health`, `self_max_health` | number | Health component values (0 if absent) |
| `has_parent` | bool | Whether this entity has a parent |
| `parent_position_x` / `_y` / `_z` | number | Parent world position |
| `is_colliding` | bool | True while this entity overlaps any collider |
| `timers_finished` | table | Array of timer names that finished this frame |

### Mouse, camera, movement

| Global | Type | Description |
|--------|------|-------------|
| `input_x`, `input_y` | number | Movement axis (-1..1) from the bound move action |
| `mouse_x`, `mouse_y` | number | Mouse screen position |
| `mouse_delta_x`, `mouse_delta_y` | number | Mouse movement since last frame |
| `mouse_scroll` | number | Scroll delta this frame |
| `mouse_left` / `mouse_right` / `mouse_middle` | bool | Button held |
| `mouse_left_just_pressed`, `mouse_right_just_pressed` | bool | Button pressed this frame |
| `camera_yaw` | number | Active camera yaw, radians |
| `camera_ev` | number | Live scene EV-100 from auto-exposure (0 if inactive) |

### Gamepad

| Global | Type | Description |
|--------|------|-------------|
| `gamepad_left_x` / `_y`, `gamepad_right_x` / `_y` | number | Stick axes (-1..1) |
| `gamepad_left_trigger`, `gamepad_right_trigger` | number | Triggers (0..1) |
| `gamepad_south` / `east` / `west` / `north` | bool | Face buttons (A/B/X/Y · Cross/Circle/Square/Triangle) |
| `gamepad_l1` / `r1` / `l2` / `r2` / `l3` / `r3` | bool | Shoulder / stick-click buttons |
| `gamepad_select`, `gamepad_start` | bool | Menu buttons |
| `gamepad_dpad_up` / `down` / `left` / `right` | bool | D-pad |

> All of the globals above are available in **both** backends except the gamepad and mouse-button set, which is only mirrored into Lua. Rhai receives the time, transform, mouse-position, `camera_yaw`/`camera_ev`, gamepad, collision, timer, health, and parent globals via its scope; use Lua for the action-map and mouse-button helpers below.

## Transform

Transform writes are queued and applied after the hook returns. **Both backends.**

| Function | Description |
|----------|-------------|
| `set_position(x, y, z)` | Set world position |
| `set_rotation(x, y, z)` | Set Euler rotation (degrees) |
| `set_scale(x, y, z)` | Set non-uniform scale |
| `set_scale_uniform(s)` | Set uniform scale |
| `translate(x, y, z)` | Move by an offset |
| `rotate(x, y, z)` | Rotate by Euler degrees |
| `look_at(x, y, z)` | Orient toward a world point |
| `parent_set_position(x, y, z)` | Set the parent's world position |
| `parent_set_rotation(x, y, z)` | Set the parent's rotation |
| `parent_translate(x, y, z)` | Move the parent by an offset |
| `set_child_position(name, x, y, z)` | Set a named child's position |
| `set_child_rotation(name, x, y, z)` | Set a named child's rotation |
| `child_translate(name, x, y, z)` | Move a named child by an offset |

## Component reflection

Read or write any registered component field by a `"Component.field"` (dot-separated) path. The setters and `get`/`get_on` are in **both** backends; the bulk/component helpers are **Lua only**.

```lua
function on_update()
    local hp = get_on("Boss", "Health.current")   -- read a field on a named entity
    set("Health.current", hp - 1)                  -- write a field on self
    if get("PhysicsReadState.grounded") then       -- read mirrored subsystem state
        apply_impulse(0, 6, 0)
    end
end
```

| Function | Backends | Description |
|----------|----------|-------------|
| `get(path)` | Both | Read a field on this entity (`nil` if missing) |
| `get_on(name, path)` | Both | Read a field on a named entity |
| `set(path, value)` | Both | Write a field on this entity |
| `set_on(name, path, value)` | Both | Write a field on a named entity |
| `get_component(type)` | Lua | Read all fields of a component as a table |
| `get_component_on(name, type)` | Lua | Same, on a named entity |
| `get_components()` | Lua | List reflected component names on self |
| `get_components_on(name)` | Lua | List component names on a named entity |
| `has_component(type)` | Lua | Test for a component on self |
| `has_component_on(name, type)` | Lua | Test for a component on a named entity |

Engine subsystems expose **read-only mirror components** through the same path mechanism: `get("PhysicsReadState.grounded")`, `get("NavReadState.*")`, `get("AnimatorReadState.*")`.

## Input

**Lua only.** The quickest inputs are the `input_x`/`input_y` and `gamepad_*` globals above; for named actions and raw keys use these functions:

```lua
function on_update()
    if is_key_just_pressed("Space") then apply_impulse(0, 8, 0) end
    if input_button_pressed("fire") then action("spawn_bullet", {}) end
    local mx, my = input_axis_2d("move")   -- returns two values
    translate(mx * 5 * delta, 0, my * 5 * delta)
end
```

| Function | Description |
|----------|-------------|
| `is_key_pressed(key)` | True while a key is held (Bevy key name, e.g. `"Space"`, `"KeyW"`) |
| `is_key_just_pressed(key)` | True the frame the key goes down |
| `is_key_just_released(key)` | True the frame the key goes up |
| `input_button_pressed(action)` | True while a mapped action is held |
| `input_button_just_pressed(action)` | True the frame the action fires |
| `input_button_just_released(action)` | True the frame the action releases |
| `input_axis_1d(action)` | 1D axis value for a mapped action |
| `input_axis_2d(action)` | 2D axis — returns `x, y` |

> Rhai registers `is_key_pressed`/`is_key_just_pressed`/`is_key_just_released`, but they take the key map as the first argument and there are no axis, action-map, or gamepad helpers. Treat input as a Lua-only feature.

## Audio

| Function | Backends | Description |
|----------|----------|-------------|
| `play_sound(path [, volume [, bus]])` | Both¹ | One-shot SFX (default bus `"Sfx"`) |
| `play_sound_looping(path, volume)` | Both | Looping SFX |
| `play_music(path [, volume [, fade_in]])` | Both¹ | Background music (bus `"Music"`) |
| `stop_music([fade_out])` | Both | Stop music with optional fade |
| `stop_all_sounds()` | Both | Stop everything |
| `play_audio([entity])` | Lua | Fire a one-shot from an entity's `AudioPlayer` (random clip + jitter); no arg = self |

¹ In Rhai, `play_sound`/`play_music` take only the `path`; use `play_sound_at_volume(path, volume)` for a volume.

## Animation

**Both backends.**

| Function | Description |
|----------|-------------|
| `play_animation(name [, looping [, speed]])` | Play a clip (defaults: looping `true`, speed `1.0`) |
| `stop_animation()` | Stop the current animation |
| `pause_animation()` | Pause playback |
| `resume_animation()` | Resume playback |
| `set_animation_speed(speed)` | Set playback speed |
| `crossfade_animation(name, duration [, looping])` | Smoothly transition to a clip |
| `set_anim_param(name, value)` | Set a float state-machine parameter |
| `set_anim_bool(name, value)` | Set a bool state-machine parameter |
| `trigger_anim(name)` | Fire a one-shot trigger parameter |
| `set_layer_weight(layer, weight)` | Set an animation layer's blend weight |

> Rhai takes only `play_animation(name)`. When `renzora_animation` is active its [extension](#extension-functions) re-registers `set_anim_param`/`set_anim_bool` (routing through `ScriptAction`) and adds `set_anim_trigger` + `get_animation_length`.

## Physics

| Function | Backends | Description |
|----------|----------|-------------|
| `apply_force(x, y, z)` | Both | Continuous force — call every frame |
| `apply_impulse(x, y, z)` | Both | One-time velocity change |
| `set_velocity(x, y, z)` | Both | Override linear velocity |
| `set_gravity_scale(scale)` | Lua | Per-body gravity multiplier |

> When `renzora_physics` is active it adds `move_controller` (collide-and-slide) and re-routes `apply_force`/`apply_impulse`/`set_linear_velocity` through its own action handlers — see [Extension functions](#extension-functions).

## Spawning & scenes

| Function | Backends | Description |
|----------|----------|-------------|
| `spawn_entity(name)` | Both | Create a new empty named entity |
| `spawn_primitive(name, kind, x, y, z [, r, g, b])` | Lua | Spawn a `ShapeRegistry` primitive (`"cube"`, `"sphere"`, …) with optional tint |
| `despawn_self()` | Both | Despawn the scripted entity |
| `despawn_by_prefix(prefix)` | Lua | Despawn every entity whose `Name` starts with `prefix` |
| `load_scene(path)` | Lua | Load a scene by path |

## Visibility, material & debug draw

| Function | Backends | Description |
|----------|----------|-------------|
| `set_visibility(visible)` | Both | Show / hide this entity |
| `set_material_color(r, g, b [, a])` | Lua | Set the base color (0..1 floats) |
| `screen_shake(intensity, duration)` | Both | Trigger a camera shake |
| `draw_line(sx, sy, sz, ex, ey, ez [, duration])` | Lua | Draw a red debug line |
| `print_log(msg)` | Both | Write to the engine console at Info level |
| `print(...)` | Both | Standard-library print |

## Cursor & environment

| Function | Backends | Description |
|----------|----------|-------------|
| `lock_cursor()` | Both | Grab and hide the cursor |
| `unlock_cursor()` | Both | Release the cursor |
| `set_sun_angles(azimuth, elevation)` | Both | Position the sun (degrees) |
| `set_fog(enabled, start, end)` | Both | Toggle and range distance fog |

## Timers

| Function | Backends | Description |
|----------|----------|-------------|
| `start_timer(name, duration [, repeat])` | Both² | Start a timer; finished names appear in `timers_finished` |
| `stop_timer(name)` | Both | Cancel a timer |

² In Rhai, `start_timer(name, duration)` is one-shot; use `start_timer_repeat(name, duration)` for repeating.

## Networking

**Lua only.** Built on the engine's Lightyear layer (native only). See [Multiplayer](/docs/r1-alpha5/multiplayer/overview).

| Function | Description |
|----------|-------------|
| `net_is_server()` | True on the dedicated/host server |
| `net_is_client()` | True when connected and not the server |
| `net_is_connected()` | True when networking is active |
| `net_player_count()` | Connected client count (server only; 0 elsewhere) |
| `rpc(name, args)` | Fire a networked RPC over the reliable channel |

```lua
function on_player_joined(id)
    rpc("welcome", { player = id })
end

function on_rpc(name, args, from)
    if name == "welcome" then print("hello " .. tostring(args.player)) end
end
```

> Connecting is done through [`action()`](#the-action-catalog), not a bare function: `action("net_connect", { address = "127.0.0.1", port = 7636 })` and `action("net_disconnect")`. `rpc()` always uses the reliable channel. Origin peer ids are lost through server relay — a client receiving another client's RPC sees `from = 0`. `net_send`, `net_send_message`, `net_spawn`, and `net_host_server` are registered but are **stubs** that never reach the wire.

## HTTP

**Lua only.** Requests are asynchronous (native only); responses arrive at `on_http` on a later frame, tagged by the callback name.

```lua
function on_ready()
    http_get("https://example.com/score", "score")
end

function on_http(callback, status, body)
    if callback == "score" and status == 200 then
        print(json_parse(body).high)
    elseif status == 0 then
        print("request failed: " .. body)
    end
end
```

| Function | Description |
|----------|-------------|
| `http_get(url [, callback])` | Fire a GET (callback defaults to `"get"`) |
| `http_post(url, body [, callback])` | POST a JSON body string (callback defaults to `"post"`) |
| `json_parse(str)` | Decode a JSON string into a table/value (`nil` on error) |

## Assets

| Function | Backends | Description |
|----------|----------|-------------|
| `asset_progress()` | Both | Returns a table `{ state, total_files, loaded_files, total_bytes, loaded_bytes, fraction, current_path, elapsed_secs }`, or `nil` when idle |
| `is_loading()` | Both | Convenience: `state == "loading"` |
| `is_loaded()` | Both | Convenience: `state == "done"` |

## Math helpers

**Both backends.** `vec2`/`vec3` return a table (`{ x, y }` / `{ x, y, z }`).

| Function | Description |
|----------|-------------|
| `vec2(x, y)` | Construct a 2D vector table |
| `vec3(x, y, z)` | Construct a 3D vector table |
| `lerp(a, b, t)` | Linear interpolation |
| `clamp(v, min, max)` | Constrain to range |

## The action() catalog

`action(name, args)` fires a generic `ScriptAction` event observed by domain crates — the escape hatch for verbs with no dedicated function. `action_on(target, name, args)` targets a named entity. **Both are Lua only.**

```lua
action("ui_set_text", { name = "score_label", text = "Score: 100" })
action("hui_spawn", { template = "ui/hud.html" })
action("net_connect", { address = "127.0.0.1", port = 7636 })
```

Verbs that are actually observed in the current code:

| Domain crate | Verbs |
|--------------|-------|
| Game UI (`renzora_game_ui`) | `ui_show`, `ui_hide`, `ui_toggle`, `ui_set_text`, `ui_set_slider`, `ui_set_checkbox`, `ui_set_toggle`, `ui_set_visible`, `ui_set_theme`, `ui_set_color` |
| Markup (`renzora_ember`) | `hui_spawn`, `hui_despawn`, `hui_hide`, `hui_show`, `quit` |
| Audio (`renzora_audio`) | `play_sound`, `play_music`, `stop_music`, `stop_all_sounds`, `play_audio_player` |
| Networking (`renzora_network`) | `net_connect`, `net_disconnect`, `net_rpc` (`net_send`, `net_send_message`, `net_spawn`, `net_host_server` are stubs) |
| Physics (`renzora_physics`) | `kinematic_slide`, `apply_force`, `apply_impulse`, `set_velocity` |
| Navmesh (`renzora_navmesh`) | `nav_set_destination`, `nav_clear_destination` |

> For widget *data* (a slider value, a bar fill), prefer reflection: `set_on("VolumeSlider", "SliderData.value", 0.5)`. Variable get/set across nodes (`global_set`/`global_get`) is a [Blueprint](/docs/r1-alpha5/scripting/blueprints) feature, not a text-script `action()` verb.

## Extension functions

Domain crates inject extra functions into the VM when their plugin is active. They register **after** the base API, so they can shadow base functions. (Lua; Rhai gets `move_controller`-family equivalents only where the crate registers them.)

| Plugin | Functions |
|--------|-----------|
| `renzora_physics` | `move_controller(x, y, z)` (collide-and-slide), plus re-registered `apply_force(x, y, z)`, `apply_impulse(x, y, z)`, `set_linear_velocity(x, y, z)` (routed through `ScriptAction`) |
| `renzora_navmesh` | `nav_set_destination(x, y, z)`, `nav_clear_destination()`, `nav_stop()` |
| `renzora_animation` | `set_anim_param(name, v)`, `set_anim_bool(name, v)`, `set_anim_trigger(name)`, `get_animation_length(name)` |

## Capabilities not exposed as functions

The `ScriptCommand` enum (`command.rs`) defines many engine verbs that have **no named function binding**. They are reachable from text scripts only via `action()`/extensions, if at all — calling them by name will fail:

`apply_torque`, `set_angular_velocity`, `Raycast`, `tween` / `tween_position` / `tween_rotation` / `tween_scale`, all particle ops (`particle_play`/`burst`/`set_rate`/…), health (`set_health`, `damage`, `heal`, `kill`, `revive`, `set_invincible`), `camera_follow` / `set_camera_target` / `set_camera_zoom`, `spawn_prefab` / `unload_scene`, sprite animation, debug draws (`draw_ray` / `draw_sphere` / `draw_box` / `draw_point`), and `set_light_intensity` / `set_light_color`.

> Do not document these as available globals — the old API draft invented names such as `rpc_send`, `is_server`, `get_network_id`, `raycast_down`, `find_entity_by_name`, `set_camera_fov`, and `terrain_get_height` that **do not exist** in the engine.

## Blueprints

Visual [Blueprints](/docs/r1-alpha5/scripting/blueprints) (`.blueprint` / `.bp`, JSON-serialized `BlueprintGraph`) are a separate system. At runtime they are **interpreted directly** by `renzora_blueprint` — walking the graph and emitting the same `ScriptAction` / transform / character commands as text scripts — not compiled to Lua. (The editor's `compile_to_lua` bake to `scripts/bp_<name>.lua` is an export action, not the live path.) Blueprints expose collision, timer, and message *events* (`event/on_collision_enter`, `event/on_timer`, `event/on_message`, …) that text scripts do not have.

## Rhai subset

Rhai (`.rhai`) is a first-class backend for the web build, but a **subset** of the Lua surface (roughly 45 vs Lua's ~70 functions) supporting only the `props`, `on_ready`, and `on_update` hooks. Compared to Lua, Rhai has **no**:

- Action-map / axis / gamepad input helpers (only raw `is_key_*` taking a key-map argument)
- Networking (`rpc`, `net_*`) or HTTP (`http_*`, `json_parse`)
- `action` / `action_on`
- Bulk reflection (`get_component*`, `has_component*`) — `get`/`set`/`get_on`/`set_on` are present
- `play_audio`, `set_gravity_scale`, `set_material_color`, `draw_line`
- `spawn_primitive`, `despawn_by_prefix`, `load_scene`

Syntax also differs:

| Feature | Lua | Rhai |
|---------|-----|------|
| Local variable | `local x = 5` | `let x = 5` |
| Map / table | `{ key = value }` | `#{ key: value }` |
| Nil / empty | `nil` | `()` |
| String concat | `..` | `+` |
| Not equal | `~=` | `!=` |
| Array index | 1-based | 0-based |
| Block end | `end` | `}` |
| Logical ops | `and` / `or` / `not` | `&&` / `\|\|` / `!` |

## See also

- [Lua](/docs/r1-alpha5/scripting/lua) — guided introduction to the full backend
- [Rhai](/docs/r1-alpha5/scripting/rhai) — the everywhere-including-web backend
- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — node graphs interpreted at runtime
- [Input Handling](/docs/r1-alpha5/scripting/input) — the action map and key names
- [Game UI](/docs/r1-alpha5/scripting/game-ui) — markup, `ui_*` verbs, and bindings
