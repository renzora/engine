# renzora Lua scripting API

Reference for the Lua scripting surface. A script is a `.lua` file attached to
an entity (via a Script component). It defines **lifecycle hooks** the engine
calls, reads **context globals** (live per-frame state), and calls **API
functions** to act on the world.

> Backend note: Lua is the native backend (`mlua`, Lua 5.4). A Rhai backend
> exists with a subset of the same surface.

---

## Lifecycle hooks (you define these)

```lua
function props()           -- declare editor-exposed / persistent variables
function on_ready()        -- once, when the entity's script starts
function on_update()       -- every frame
function on_rpc(name, args, from)        -- networked RPC received
function on_ui(name, args, entity)       -- markup callback (on_press, etc.)
function on_http(name, code, body)       -- HTTP response (http_get/http_post)
function on_player_joined(id)            -- server: a player connected
function on_player_left(id)              -- server: a player disconnected
```

### `props()`
Return a table declaring variables. They show in the inspector and **persist**:
their runtime values are read back from the VM each frame, so a UI template can
bind them (`{{ Entity.varname }}`).

```lua
function props()
    return {
        speed   = { value = 5.0,  hint = "Move speed" },
        enabled = { value = true },
        label   = { value = "hi" },
        _t      = { value = 0.0,  hint = "Internal (underscore = hidden-ish)" },
    }
end
```
The parser reads `value` (or `default`), plus optional `hint` and `tab`. The
type is **inferred from the value**, so a `type` field is ignored (cosmetic /
self-documentation only). Inferred types: `float`, `int`, `bool`, `string`,
`vec2`, `vec3`, `color`.

---

## Context globals (read-only, refreshed each frame)

### Time
- `delta` — seconds since last frame
- `elapsed` — seconds since start

### Self transform
- `position_x` `position_y` `position_z`
- `rotation_x` `rotation_y` `rotation_z`
- `scale_x` `scale_y` `scale_z`

### Parent
- `has_parent`
- `parent_position_x` `parent_position_y` `parent_position_z`

### Self entity
- `self_entity_id`, `self_entity_name`
- `self_health`, `self_max_health`

### Mouse
- `mouse_x` `mouse_y` — cursor position
- `mouse_delta_x` `mouse_delta_y` — movement this frame
- `mouse_left` `mouse_right` `mouse_middle` — held
- `mouse_left_just_pressed` `mouse_right_just_pressed`
- `mouse_scroll`

### Movement / input axes (combined keyboard + gamepad)
- `input_x` `input_y` — normalized movement intent

### Gamepad
- Sticks: `gamepad_left_x` `gamepad_left_y` `gamepad_right_x` `gamepad_right_y`
- Triggers: `gamepad_left_trigger` `gamepad_right_trigger`
- Buttons (bool): `gamepad_south` `gamepad_east` `gamepad_west` `gamepad_north`
  `gamepad_l1` `gamepad_r1` `gamepad_l2` `gamepad_r2` `gamepad_l3` `gamepad_r3`
  `gamepad_select` `gamepad_start`
- D-pad: `gamepad_dpad_up` `gamepad_dpad_down` `gamepad_dpad_left` `gamepad_dpad_right`

### Camera
- `camera_yaw`, `camera_ev` (auto-exposure EV-100)

### Physics / misc
- `is_colliding`
- `timers_finished` — set when a `start_timer` elapses (see Timers)

---

## API functions

### Transform (self)
```lua
set_position(x, y, z)        set_rotation(x, y, z)      -- euler degrees
set_scale(x, y, z)           set_scale_uniform(s)
translate(x, y, z)           rotate(x, y, z)
look_at(x, y, z)
```

### Transform (parent / children)
```lua
parent_set_position(x, y, z)   parent_set_rotation(x, y, z)   parent_translate(x, y, z)
set_child_position(name, x, y, z)   set_child_rotation(name, x, y, z)   child_translate(name, x, y, z)
```

### Reflection — read/write any component field
```lua
get("Component.field")                      -- read on self -> value
set("Component.field", value)               -- write on self
get_on("EntityName", "Component.field")     -- read on a named entity
set_on("EntityName", "Component.field", value)
get_component("Component")                   -- whole component as a table
get_component_on("EntityName", "Component")
get_components()                             -- list component type names on self
get_components_on("EntityName")
has_component("Component")                   -- bool
has_component_on("EntityName", "Component")
```
`value` accepts numbers, bools, strings, and `{x=,y=,z=}` / `{r=,g=,b=,a=}`
tables for vectors/colors. Field paths support nesting: `Transform.translation.x`,
`Sun.color.x`.

### Input (action/key/button)
```lua
is_key_pressed("KeyW")          is_key_just_pressed("Space")     is_key_just_released("Escape")
input_button_pressed("name")    input_button_just_pressed("name")  input_button_just_released("name")
input_axis_1d("name")           -- -> float
input_axis_2d("move")           -- -> x, y (two return values)
```
Key names match Bevy `KeyCode` (`KeyW`, `ArrowUp`, `Space`, `Enter`, `Escape`,
`ShiftLeft`, …). Actions/buttons work for keyboard + gamepad uniformly.

### Audio
```lua
play_sound(path [, volume] [, bus])     -- one-shot SFX (bus default "Sfx")
play_sound_looping(path, volume)
play_music(path [, volume] [, fade_in]) -- looped, bus "Music"
stop_music([fade_out])
stop_all_sounds()
play_audio([entity_name])               -- fire an entity's AudioPlayer pool
```

### Animation
```lua
play_animation(name)     stop_animation()     pause_animation()     resume_animation()
set_animation_speed(speed)
crossfade_animation(name, duration)
-- animation graph / state machine params:
set_anim_bool(name, value)     set_anim_param(name, value)     trigger_anim(name)
set_layer_weight(layer, weight)
```

### Spawning & scene
```lua
spawn_entity(...)            -- spawn an entity
spawn_primitive(...)         -- spawn a primitive shape
despawn_self()
despawn_by_prefix(prefix)    -- despawn entities whose name starts with prefix
load_scene(path)
```

### Physics
```lua
apply_force(...)     apply_impulse(...)     set_velocity(...)     set_gravity_scale(scale)
```

### Visibility / material
```lua
set_visibility(visible)               -- self
set_material_color(...)
```

### Environment
```lua
set_sun_angles(azimuth, elevation)
set_fog(...)
```

### Timers
```lua
start_timer(name, duration [, repeat])
stop_timer(name)
-- when a timer elapses, `timers_finished` reflects it for that frame
```

### Cursor / camera fx
```lua
lock_cursor()     unlock_cursor()
screen_shake(...)
```

### Debug / visualization
```lua
draw_line(start_x, start_y, start_z, end_x, end_y, end_z [, duration])
```
Draws a debug line between two world-space points (currently red). `duration`
is seconds; omit (or `0`) for a single-frame line.

### Networking (see also on_rpc / on_player_*)
```lua
rpc(name, args)            -- send an RPC to peers
net_is_server()            net_is_client()      net_is_connected()
net_player_count()
```

### HTTP (async)
```lua
http_get(url [, callback])           -- callback default "get"
http_post(url, body [, callback])    -- body = JSON string; callback default "post"
json_parse(str)                      -- JSON string -> Lua table/value (nil on error)
-- result delivered to: function on_http(name, code, body)
```
Requests run on a background thread; the game loop never blocks. The handling
script typically `json_parse`s the body and stores a value in a `props()`
variable, which a UI template then binds.

### Generic actions (domain events)
```lua
action(name [, args_table])             -- trigger a ScriptAction observed by domain crates
action_on(entity_name, name [, args])   -- targeted at another entity
```
Used for engine verbs without a dedicated function (e.g. UI spawn:
`action("hui_spawn", { template = "templates/menu.html" })`).

### Assets
```lua
is_loaded()        is_loading()
asset_progress()   -- table: { state, fraction, loaded_files, total_files, ... }
```

### Logging & math helpers
```lua
print(...)            -- standard Lua print
print_log(msg)        -- engine log
clamp(v, min, max)    lerp(a, b, t)
vec2(x, y)            vec3(x, y, z)     -- construct vector tables
```

---

## Patterns

### Read input, move
```lua
function on_update()
    local mx, my = input_axis_2d("move")
    translate(mx * speed * delta, 0, my * speed * delta)
end
```

### Drive another entity by name
```lua
function on_update()
    set_on("World Environment", "Sun.azimuth", (elapsed * 10) % 360)
end
```

### HTTP + UI binding
```lua
function on_ready() http_post("https://api.example.com/login", body, "login") end
function on_http(name, code, body)
    if name == "login" and code == 200 then
        local d = json_parse(body)
        username = d.user.username   -- a props() var -> bind {{ Entity.username }}
    end
end
```

### UI callback
```lua
-- markup: <button on_press="start">Play</button>
function on_ui(name)
    if name == "start" then load_scene("scenes/level1.scn") end
end
```

> This reference is generated from the registered Lua surface in
> `crates/renzora_scripting/src/backends/lua.rs`. Exact argument lists for a
> few verbs (spawn/physics/animation) may have additional optional params;
> check the source if a call is rejected.
