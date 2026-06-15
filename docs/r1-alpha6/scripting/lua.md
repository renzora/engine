# Lua

Write per-entity gameplay logic in Lua 5.4 — the full-featured scripting backend for native desktop and mobile builds.

Renzora's scripting core is language-agnostic: the backend is chosen by file extension. A `.lua` file runs on the **Lua** backend (mlua 0.10, Lua 5.4, vendored), and a `.rhai` file runs on **Rhai**. Lua is compiled into every native build but is **not** available on the web (wasm) target — use [Rhai](/docs/r1-alpha5/scripting/rhai) there. Lua exposes the full API (~70 functions and all eight lifecycle hooks); Rhai is a deliberate subset.

A `ScriptComponent` is auto-inserted on any entity that receives a `Name`, so naming an entity in the editor is all you need before attaching a script to it. One Lua VM is cached per `(entity, script)` and persists across frames.

## Your first script

Create `scripts/player.lua`:

```lua
function on_ready()
    print("Player spawned!")
end

function on_update()
    local speed = 5.0
    translate(input_x * speed * delta, 0, input_y * speed * delta)
end
```

Name an entity in the Inspector, add the script under its `ScriptComponent`, and hit play — the entity moves with the movement input axis (WASD / left stick).

> The transform globals (`position_x`, `rotation_y`, …) are **read-only inputs** refreshed each frame. Assigning to them does nothing. To move an entity, call a transform function such as `translate(x, y, z)` or `set_position(x, y, z)`, or drive it through physics (`set_velocity`, `apply_impulse`).

## Lifecycle hooks

Define any of these free functions; the engine calls the ones that exist. None are required.

| Hook | When it fires |
|------|---------------|
| `props()` | Once on load — returns a table of editable properties (see below) |
| `on_ready()` | Once, the first frame the script runs |
| `on_update()` | Every frame |
| `on_rpc(name, args, from)` | A networked RPC arrived. `from` is the sender's peer id (`0` for relayed messages) |
| `on_ui(name, args, entity)` | A markup UI event fired (`on_press`, `on_change`, …). `entity` is the firing node's id as a **u64 integer** (`Entity::to_bits()`), not a handle |
| `on_http(callback, status, body)` | An HTTP response came back. `status` is the HTTP code; **`status == 0` means the request failed and `body` holds the error text** |
| `on_player_joined(id)` | A player connected (fires on the **server/host only**) |
| `on_player_left(id)` | A player disconnected (server/host only) |

> There is **no** `on_start`, `on_collision`, or `on_destroy` hook. Use `on_ready` for setup, and read the `is_colliding` global for overlap state. Collision *events* are available only as [Blueprint](/docs/r1-alpha5/scripting/blueprints) nodes.

## Script properties

`props()` returns a table of variables that appear as editable fields in the Inspector, so designers can tune values without touching code:

```lua
function props()
    return {
        speed     = { value = 5.0, hint = "Movement speed in m/s" },
        jump      = { default = 10.0, tab = "Movement" },
        can_fly   = false,
        team_name = "Red",
    }
end

function on_update()
    translate(input_x * speed * delta, 0, input_y * speed * delta)
end
```

- Each property is either a bare value or a table with `value` (or `default`), optional `hint`, and optional `tab` (groups fields under a named tab in the Inspector).
- The widget type is **inferred from the value** — number, bool, string, entity, vec2/vec3, or color. There is no `type` field, and `min`/`max` keys are not read.
- Declared properties become readable/writable globals inside every hook. After each hook the engine reads the globals back, so changes persist and can be bound into UI with `{{ Entity.speed }}`.

## Reading the world

These globals are set fresh before each hook. They are inputs — read them, don't assign to them.

| Global | Type | Description |
|--------|------|-------------|
| `delta` | number | Seconds since the previous frame |
| `elapsed` | number | Seconds since startup |
| `position_x` / `_y` / `_z` | number | World position (read-only) |
| `rotation_x` / `_y` / `_z` | number | Euler rotation in degrees (read-only) |
| `scale_x` / `_y` / `_z` | number | World scale (read-only) |
| `input_x`, `input_y` | number | Movement axis (-1..1) from the bound move action |
| `mouse_x`, `mouse_y` | number | Mouse screen position |
| `mouse_delta_x`, `mouse_delta_y` | number | Mouse movement since last frame |
| `mouse_scroll` | number | Scroll delta this frame |
| `mouse_left` / `mouse_right` / `mouse_middle` | bool | Button held |
| `mouse_left_just_pressed`, `mouse_right_just_pressed` | bool | Button pressed this frame |
| `camera_yaw` | number | Active camera yaw (radians) |
| `camera_ev` | number | Live scene EV-100 from auto-exposure (0 if inactive) |
| `gamepad_left_x` / `_y`, `gamepad_right_x` / `_y` | number | Stick axes |
| `gamepad_left_trigger`, `gamepad_right_trigger` | number | Triggers (0..1) |
| `gamepad_south` / `east` / `west` / `north` | bool | Face buttons |
| `gamepad_l1` / `r1` / `l2` / `r2` / `l3` / `r3` | bool | Shoulder / stick buttons |
| `gamepad_select`, `gamepad_start` | bool | Menu buttons |
| `gamepad_dpad_up` / `down` / `left` / `right` | bool | D-pad |
| `is_colliding` | bool | True while this entity overlaps any collider |
| `timers_finished` | table | Array of timer names that finished this frame |
| `self_entity_id` | integer | This entity's id (bits) |
| `self_entity_name` | string | This entity's `Name` |
| `self_health`, `self_max_health` | number | Health component values (0 if absent) |
| `has_parent` | bool | Whether this entity has a parent |
| `parent_position_x` / `_y` / `_z` | number | Parent world position |

## Input

Two input styles are available. The `input_x`/`input_y` globals and `gamepad_*` globals above are the quickest; for named actions and raw keys, use these functions:

```lua
function on_update()
    if is_key_just_pressed("Space") then
        apply_impulse(0, 8, 0)            -- jump
    end

    -- Action map: same code works on keyboard and gamepad
    if input_button_pressed("fire") then
        action("spawn_bullet", {})
    end

    local mx, my = input_axis_2d("move")  -- returns two values
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

## Moving and transforming

Transform writes are queued as commands and applied after the hook returns.

```lua
function on_update()
    translate(0, 0, -5 * delta)   -- move forward in local space
    rotate(0, 90 * delta, 0)      -- spin 90°/s around Y
    look_at(0, 1, 0)              -- face a world point
end
```

| Function | Description |
|----------|-------------|
| `set_position(x, y, z)` | Set world position |
| `set_rotation(x, y, z)` | Set Euler rotation (degrees) |
| `set_scale(x, y, z)` / `set_scale_uniform(s)` | Set scale |
| `translate(x, y, z)` | Move by an offset |
| `rotate(x, y, z)` | Rotate by Euler degrees |
| `look_at(x, y, z)` | Orient toward a world point |
| `parent_set_position` / `parent_set_rotation` / `parent_translate` | Same, applied to the parent |
| `set_child_position(name, x, y, z)` / `set_child_rotation(...)` / `child_translate(...)` | Apply to a named child |

## Physics, audio, animation, environment

```lua
function on_ready()
    play_music("audio/theme.ogg", 0.6)
    play_animation("idle", true, 1.0)   -- name, looping, speed
end
```

| Category | Functions |
|----------|-----------|
| Physics | `apply_force(x,y,z)`, `apply_impulse(x,y,z)`, `set_velocity(x,y,z)`, `set_gravity_scale(s)` |
| Audio | `play_sound(path[, vol[, bus]])`, `play_sound_looping(path, vol)`, `play_music(path[, vol[, fade]])`, `stop_music([fade])`, `stop_all_sounds()`, `play_audio([entity])` |
| Animation | `play_animation(name[, looping[, speed]])`, `stop_animation()`, `pause_animation()`, `resume_animation()`, `set_animation_speed(s)`, `seek_animation(t)`, `get_animation_time()`, `is_animation_playing()`, `crossfade_animation(name, dur[, looping])`, `set_anim_param(name, v)`, `set_anim_bool(name, v)`, `trigger_anim(name)`, `set_layer_weight(layer, w)`. Hook: `on_animation_event(name, entity)` on clip markers. |
| Timers | `start_timer(name, duration[, repeat])`, `stop_timer(name)` — finished timers appear in `timers_finished` |
| Spawning | `spawn_entity(name)`, `spawn_primitive(name, kind, x, y, z[, r, g, b])`, `despawn_self()`, `despawn_by_prefix(prefix)`, `load_scene(path)` |
| Rendering | `set_visibility(bool)`, `set_material_color(r, g, b[, a])`, `screen_shake(intensity, duration)`, `draw_line(sx,sy,sz, ex,ey,ez[, duration])` |
| Environment | `set_sun_angles(azimuth, elevation)`, `set_fog(enabled, start, end)` |
| Cursor | `lock_cursor()`, `unlock_cursor()` |
| Math | `vec2(x, y)`, `vec3(x, y, z)`, `lerp(a, b, t)`, `clamp(v, min, max)` |
| Assets | `asset_progress()`, `is_loading()`, `is_loaded()` |
| Logging | `print(...)` (stdlib), `print_log(msg)` (engine console) |

## Reading and writing components

The reflection functions read or write any registered component field by a `"Component.field"` path:

```lua
function on_update()
    -- Read another entity's health
    local hp = get_on("Boss", "Health.current")

    -- Write a field on this entity
    set("Health.current", hp - 1)

    -- Read engine subsystem state mirrored onto this entity
    if get("PhysicsReadState.grounded") then
        apply_impulse(0, 6, 0)
    end
end
```

| Function | Description |
|----------|-------------|
| `get(path)` / `get_on(name, path)` | Read a field on this / a named entity |
| `set(path, value)` / `set_on(name, path, value)` | Write a field |
| `get_component(type)` / `get_component_on(name, type)` | Read all fields of a component as a table |
| `get_components(...)` / `get_components_on(...)` | Read multiple components at once |
| `has_component(type)` / `has_component_on(name, type)` | Test for a component |

Engine subsystems mirror read-only state through this same mechanism: `get("PhysicsReadState.grounded")`, `get("NavReadState.*")`, and `get("AnimatorReadState.*")`.

## Networking

Multiplayer scripting is native-only and built on the engine's Lightyear layer. Hooks `on_rpc`, `on_player_joined`, and `on_player_left` deliver events; these functions query and send:

```lua
function on_update()
    if net_is_server() then
        -- server-authoritative logic
    end
end

function on_player_joined(id)
    rpc("welcome", { player = id })   -- broadcast an RPC
end

function on_rpc(name, args, from)
    if name == "welcome" then
        print("welcomed " .. tostring(args.player))
    end
end
```

| Function | Description |
|----------|-------------|
| `net_is_server()` | True on the dedicated/host server |
| `net_is_client()` | True when connected and not the server |
| `net_is_connected()` | True when networking is active |
| `net_player_count()` | Connected client count (server only; 0 elsewhere) |
| `rpc(name, args)` | Fire a networked RPC (reliable channel) |

> Connecting is done through `action()`, not a bare function: `action("net_connect", { address = "127.0.0.1", port = 7636 })` and `action("net_disconnect")`. Origin peer ids are lost through server relay — a client receiving another client's RPC sees `from = 0`. See [Multiplayer](/docs/r1-alpha5/multiplayer/overview).

## HTTP

Requests are asynchronous (native only). Responses are delivered to `on_http` on a later frame, tagged by the callback name you pass:

```lua
function on_ready()
    http_get("https://example.com/score", "score")
end

function on_http(callback, status, body)
    if callback == "score" and status == 200 then
        local data = json_parse(body)
        print(data.high)
    elseif status == 0 then
        print("request failed: " .. body)
    end
end
```

`http_get(url[, callback])`, `http_post(url, body[, callback])`, and `json_parse(str)` are the full surface. The callback defaults to `"get"` / `"post"`.

## The action() escape hatch

`action(name, args)` fires a generic `ScriptAction` event that domain crates observe. It's how scripts reach a large catalog of verbs that have no dedicated function — UI widgets, markup, audio players, globals, networking, and more:

```lua
action("ui_set_text", { name = "score_label", text = "Score: 100" })
action("hui_spawn", { template = "ui/hud.html" })
action("global_set", { key = "coins", value = 5 })
```

`action_on(target, name, args)` targets a named entity instead of self. Common families include 40+ `ui_*` widget verbs, `hui_*` markup verbs, `global_set`/`global_get`, `net_connect`/`net_disconnect`, and `play_audio_player`.

## Extension functions

Some domain crates inject extra functions into the Lua VM when their plugin is active. They are available exactly like the built-ins:

| Plugin | Functions |
|--------|-----------|
| `renzora_physics` | `move_controller(...)`, `kinematic_slide(...)` |
| `renzora_navmesh` | `nav_set_destination(x, y, z)`, `nav_clear_destination()`, `nav_stop()` |
| `renzora_animation` | `set_anim_param`, `set_anim_bool`, `set_anim_trigger`, `get_animation_length` |

## Lua vs Rhai

You can mix `.lua` and `.rhai` scripts in one project; the backend is picked per file by extension. Lua and Rhai are **not** interchangeable — Rhai is a subset (~45 of Lua's ~70 functions, and only the `props`, `on_ready`, and `on_update` hooks). Rhai has no input, networking, HTTP, `action()`, component reflection, or child-transform functions. Choose Lua for full-featured native games; choose Rhai when you need scripts on the web build.

The languages also differ in syntax:

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

## What's next

- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts attach, run, and dispatch by extension
- [Rhai](/docs/r1-alpha5/scripting/rhai) — the everywhere-including-web backend
- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — node graphs interpreted at runtime
- [Input Handling](/docs/r1-alpha5/scripting/input) — the action map and key names
- [Scripting API](/docs/r1-alpha5/api/scripting) — the full function reference
