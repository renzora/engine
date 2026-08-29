# Scripting API

The authoritative reference for every global, function, lifecycle hook, and `action()` verb exposed to scripts.

This page documents the surface registered by `crates/renzora_scripting` plus the functions declared by domain crates. For a guided introduction see [Lua](/docs/r1-alpha7/scripting/lua) and the [Scripting Overview](/docs/r1-alpha7/scripting/overview).

## How the API is dispatched

The scripting core is language-agnostic. `crates/renzora_scripting` owns the hooks, the command vocabulary, the context and the queue that applies commands to the world — and contains no interpreter. A `ScriptEngine` resource holds a list of backends and routes each script to one **by file extension**:

| Extension | Backend | Where it comes from |
|-----------|---------|---------------------|
| `.lua` | Lua (mlua, Lua 5.4, vendored) | `plugins/lua` — a standalone C-ABI plugin |
| `.rs` | Rust | `crates/renzora_rust_script` — compiled to a native plugin per script |

Which language a game can be scripted in is decided by **which plugin is present**, not by how the engine was compiled. Removing `plugins/lua` removes Lua; adding a backend plugin adds a language. Two languages coexist in one project.

> **Rhai has been removed.** Earlier releases shipped a second `.rhai` backend that was a subset of the Lua surface. It is gone: there is no `.rhai` backend, no Rhai crate, and no "both backends" distinction. Everything on this page is the Lua surface. If you have `.rhai` scripts, port them to Lua — the function names are almost all identical, and the [syntax differences](#porting-from-rhai) are listed at the end.

Every callable here is one of two things:

- A **registered function** — a closure exposed to the VM that pushes a `ScriptCommand` onto a per-frame queue, applied after the hook returns.
- A **context global** — a value written fresh into the VM before each hook. Globals are inputs; assigning to them has no effect.

A `ScriptComponent` is never inserted automatically. Attach a script from the inspector's **Scripts** section — shown on every entity, so there is nothing to add first — or by dropping a script file onto the entity's row in the hierarchy, and the component appears with the script already on it. Removing the last script removes the component again, so the always-visible section costs nothing on an entity you never scripted. Authored game UI is the one exception: `renzora_ember::game_ui` gives every `UiWidget`/`UiCanvas` a `ScriptComponent` so `<input bind="Entity.var">` has something to resolve against. One Lua VM is cached per `(entity, script_path)` and reused across frames.

## Lifecycle hooks

Define any of these free functions; the engine calls the ones that exist. None are required.

| Hook | Fires when |
|------|-----------|
| `props()` | Once on load — returns a table of editable Inspector properties |
| `on_ready()` | Once, the first frame the script runs |
| `on_update()` | Every frame |
| `on_draw(g)` | Every frame, for an entity with a [canvas surface](#on_draw--the-2d-canvas) |
| `on_rpc(name, args, from)` | A networked RPC arrived. `from` is the sender's peer id (`0` for relayed messages) |
| `on_ui(name, args, entity)` | A markup UI event fired. `entity` is the firing node's `Entity::to_bits()` as a **u64 integer**, not a handle |
| `on_animation_event(name, entity)` | Playback crossed a named clip marker |
| `on_http(callback, status, body)` | An HTTP response returned. `status` is the HTTP code; **`status == 0` means the request failed** and `body` holds the error text |
| `on_player_joined(id)` | A player connected (**server/host only**) |
| `on_player_left(id)` | A player disconnected (server/host only) |
| `on_scene_loaded(path)` | A scene finished loading. Only scripts that **survive** the load hear this — see below |
| `on_scene_load_failed(path, error)` | A scene load failed. `error` is the reason |
| `on_event(name, args)` | A broadcast game event was emitted, by any script, blueprint or Rust system |

Hooks are selected by op code across the plugin boundary rather than by name, so adding a hook is not an ABI break.

> **The scene hooks only reach `Persistent` scripts.** A scene load despawns the outgoing scene's entities partway through, so a script that lives in the scene being replaced is already gone when its successor arrives. Put scene-transition logic on a global scene (see [Global scenes](../engine-core/resources#cross-scene-state)) — that is the entire reason a loading screen has to live outside the scene it is covering.

> There is **no** `on_start`, `on_collision`, or `on_destroy` hook. Use `on_ready` for setup and read the `is_colliding` global for overlap state. Collision *events* exist only as [Blueprint](/docs/r1-alpha7/scripting/blueprints) nodes.

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

## on_draw — the 2D canvas

`on_draw(g)` is an immediate-mode 2D drawing pass, and the one hook that is not called for every scripted entity: it runs only for an entity that owns a **canvas surface**, and is sized to it.

A canvas is a markup node with the `canvas` attribute:

```html
<node canvas width="300px" height="300px" />
```

The script attached to that node's binding host then paints into it:

```lua
function on_draw(g)
    local cx, cy = g.width / 2, g.height / 2
    g.circle(cx, cy, 100, "#1b1b22")
    g.arc(cx, cy, 92, -90, -90 + 270 * fuel, "#ffb347", 8)
    g.text(cx, cy + 6, string.format("%d%%", fuel * 100), 22, "#ffffff")
end
```

`g` carries `g.width` and `g.height` — the surface's size in pixels — plus the shape methods below. **Call them with dot syntax** (`g.circle(...)`, not `g:circle(...)`): they take no `self`.

| Method | Arguments |
|---|---|
| `g.line(x1, y1, x2, y2, color [, thickness])` | thickness defaults to 2 |
| `g.arc(cx, cy, r, start, end, color [, thickness])` | angles in **degrees**, 0 = +x, clockwise |
| `g.circle(cx, cy, r, color)` | filled |
| `g.rect(x, y, w, h, color)` | filled |
| `g.triangle(x1, y1, x2, y2, x3, y3, color)` | filled |
| `g.poly(points, color)` | filled polygon |
| `g.text(x, y, text, size, color)` | baseline-anchored at `(x, y)`, centred horizontally on `x` |

Coordinates are the canvas's local pixels, **top-left origin, y-down** — screen convention, not the y-up of the 3D scene. Colours are `#rrggbb` / `#rrggbbaa` hex **strings**, not tables.

The list is rebuilt from scratch every frame and reconciled into a pool of existing SDF shape entities parented under the canvas node — reused in place rather than respawned, and z-ordered by draw index, so a needle drawn after its dial sits on top. That means `on_draw` is cheap to call every frame and you should not try to cache anything yourself; just describe the picture.

This is the right tool for a gauge, a minimap, a radar, a health arc, a custom graph — anything that is drawing rather than layout. For a HUD made of *widgets*, use [Game UI](/docs/r1-alpha7/scripting/game-ui) markup instead.

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
| `project_width`, `project_height` | number | Configured game resolution in world units (falls back to 1920×1080 with no project loaded) |

### Gamepad

| Global | Type | Description |
|--------|------|-------------|
| `gamepad_left_x` / `_y`, `gamepad_right_x` / `_y` | number | Stick axes (-1..1) |
| `gamepad_left_trigger`, `gamepad_right_trigger` | number | Triggers (0..1) |
| `gamepad_south` / `east` / `west` / `north` | bool | Face buttons (A/B/X/Y · Cross/Circle/Square/Triangle) |
| `gamepad_l1` / `r1` / `l2` / `r2` / `l3` / `r3` | bool | Shoulder / stick-click buttons |
| `gamepad_select`, `gamepad_start` | bool | Menu buttons |
| `gamepad_dpad_up` / `down` / `left` / `right` | bool | D-pad |

The flat `gamepad_*` globals mirror the **first connected pad**. Every pad is addressable by stable slot id (0 = first):

| Function | Description |
|----------|-------------|
| `gamepad_count()` | Connected pads |
| `gamepad_connected(pad)` | Pad id present |
| `gamepad_axis(pad, axis)` | `"left_x"`, `"right_y"`, `"left_trigger"`, … |
| `gamepad_left_stick(pad)` / `gamepad_right_stick(pad)` | Returns two values, `x, y` |
| `gamepad_button(pad, button)` | `"south"`, `"l1"`, `"dpad_up"`, … |
| `gamepad_button_just_pressed(pad, button)` | Down this frame |

See [Input Handling — Multiple gamepads](/docs/r1-alpha7/scripting/input#multiple-gamepads).

## Transform

Transform writes are queued and applied after the hook returns.

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
| `goto_camera_preset(name)` | Jump self to a named camera angle (see below) |

## Camera presets

A camera entity can carry a list of named angles in a `CameraPresets` component. Author them in the inspector's **Camera Presets** section — *Capture current view* snapshots the editor fly-camera's pose (parent-aware) into a new preset, and each row offers rename / go-to / delete. Presets serialize into the scene.

Attach a script to that camera and jump between angles by name:

```lua
function on_update()
    if pressed("aim") then
        goto_camera_preset("over_shoulder")
    elseif pressed("map") then
        goto_camera_preset("top_down")
    end
end
```

`goto_camera_preset(name)` moves the script's **own** entity to the matching preset's stored translation + rotation (it's a transform write, applied after the hook returns). It's a no-op with a console warning if the entity has no `CameraPresets` or no preset matches `name`. To read the list generically, use component reflection (`get("CameraPresets...")`).

## Field of view

FOV lives inside Bevy's `Projection` **enum**, which the generic `get`/`set` reflect paths cannot address — so it gets two declared functions instead. Both are degrees, matching the inspector's FOV field.

| Function | Description |
|----------|-------------|
| `set_fov(degrees)` | Set this camera's vertical field of view. Clamped to 10–170, the same bounds the inspector enforces. A no-op on an orthographic camera. |
| `camera_fov()` | This camera's vertical field of view in degrees, or **0** if it is orthographic (so a script can tell the difference rather than reading a fake angle). |

```lua
local base_fov = 0.0

function on_ready()
    base_fov = camera_fov()
end

function on_update()
    -- Breathe the lens by half a degree.
    set_fov(base_fov + math.sin(elapsed * 0.4) * 0.5)
end
```

`camera_fov()` reads a `CameraReadState` mirror that `renzora_engine` refreshes each frame from the projection; it is never saved into a scene. See `assets/scripts/camera_sway.lua` for both functions in use — it breathes the lens *and* scales its rotation amplitudes by FOV, so the sway covers the same fraction of the frame on any lens.

## Component reflection

Read or write any registered component field by a `"Component.field"` (dot-separated) path.

```lua
function on_update()
    local hp = get_on("Boss", "Health.current")   -- read a field on a named entity
    set("Health.current", hp - 1)                  -- write a field on self
    if get("PhysicsReadState.grounded") then       -- read mirrored subsystem state
        apply_impulse(0, 6, 0)
    end
end
```

| Function | Description |
|----------|-------------|
| `get(path)` | Read a field on this entity (`nil` if missing) |
| `get_on(name, path)` | Read a field on a named entity |
| `set(path, value)` | Write a field on this entity |
| `set_on(name, path, value)` | Write a field on a named entity |
| `get_component(type)` | Read all fields of a component as a table |
| `get_component_on(name, type)` | Same, on a named entity |
| `get_components()` | List reflected component names on self |
| `get_components_on(name)` | List component names on a named entity |
| `has_component(type)` | Test for a component on self |
| `has_component_on(name, type)` | Test for a component on a named entity |

Engine subsystems expose **read-only mirror components** through the same path mechanism: `get("PhysicsReadState.grounded")`, `get("NavReadState.*")`, `get("AnimatorReadState.*")`, `get("ParkourReadState.*")`, `get("WindState.speed")`.

## Input

The quickest inputs are the `input_x`/`input_y` and `gamepad_*` globals above; for named actions and raw keys use these functions:

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

## Audio

| Function | Description |
|----------|-------------|
| `play_sound(path [, volume [, bus]])` | One-shot SFX (default bus `"Sfx"`) |
| `play_sound_looping(path, volume)` | Looping SFX |
| `play_music(path [, volume [, fade_in]])` | Background music (bus `"Music"`) |
| `stop_music([fade_out])` | Stop music with optional fade |
| `stop_all_sounds()` | Stop everything |
| `play_audio([entity])` | Fire a one-shot from an entity's `AudioPlayer` (random clip + jitter); no arg = self |

## Animation

| Function | Description |
|----------|-------------|
| `play_animation(name [, looping [, speed]])` | Play a clip — bone **and** property tracks (defaults: looping `true`, speed `1.0`) |
| `stop_animation()` | Stop the current animation |
| `pause_animation()` | Pause playback |
| `resume_animation()` | Resume playback |
| `set_animation_speed(speed)` | Set playback speed |
| `seek_animation(time)` | Jump playback to `time` seconds |
| `get_animation_time()` | Current property-playback time (seconds) |
| `is_animation_playing()` | `true` unless paused or stopped |
| `crossfade_animation(name, duration [, looping])` | Smoothly transition to a clip |
| `set_anim_param(name, value)` | Set a float state-machine parameter |
| `set_anim_bool(name, value)` | Set a bool state-machine parameter |
| `trigger_anim(name)` / `set_anim_trigger(name)` | Fire a one-shot trigger parameter |
| `set_layer_weight(layer, weight)` | Set an animation layer's blend weight |
| `get_animation_length(name)` | Clip length in seconds |

**2D sprite animations too.** [Sprite-sheet clips](../editor/sprite-animation.md) are ordinary property clips (a `SpriteSheet.frame` track), so the same calls drive them: `play_animation("run")` works whether "run" is a bone clip or a sprite clip — the entity has one animator either way.

`on_animation_event(name, entity)` fires when playback crosses a named clip marker (see [Animation → Event markers](../editor/animation.md#event-markers)). Markers fire in play mode and in exported games, and loop-wrap is handled.

## Physics

| Function | Description |
|----------|-------------|
| `apply_force(x, y, z)` | Continuous force — call every frame |
| `apply_impulse(x, y, z)` | One-time velocity change |
| `set_velocity(x, y, z)` | Override linear velocity |
| `set_gravity_scale(scale)` | Per-body gravity multiplier |
| `move_controller(x, y, z)` | Collide-and-slide character movement (needs `renzora_physics`) |

## Parkour

Provided by `renzora_parkour`. These **replace** `move_controller` for the entity: the parkour controller owns gravity, ground contact and collision itself, because a ledge hang and a rope swing are positions it has to set rather than forces it can ask for. Driving one entity with both fights over its `Transform`.

| Function | Description |
|----------|-------------|
| `parkour_move(x, y, z)` | Movement intent in world space; `x`/`z` steer, `y` climbs ladders and hangs. Consumed each frame, so call it every frame |
| `parkour_sprint(on)` | Move at `run_speed` instead of `walk_speed` |
| `parkour_jump()` | Jump, wall-jump, climb up from a hang, or let go of a swing with a boost |
| `parkour_action()` | Context traversal: vault, mantle, grab a ledge, mount a ladder, grab a rope |
| `parkour_release()` | Let go of a ledge, ladder or swing |

Reads go through reflection: `get("ParkourReadState.state")`, `.event`, `.grounded`, `.speed`, `.traversing`, `.can_vault`, `.can_mantle`, `.can_grab`, `.near_ladder`, `.ledge_height`. See [Parkour & Traversal](../scripting/parkour.md).

## Navigation

Provided by `renzora_navmesh`.

| Function | Description |
|----------|-------------|
| `nav_set_destination(x, y, z)` | Path to a world point |
| `nav_clear_destination()` | Drop the current path |
| `nav_stop()` | Stop moving, keep the path |

## Spawning & scenes

| Function | Description |
|----------|-------------|
| `spawn_entity(name)` | Create a new empty named entity |
| `spawn_primitive(name, kind, x, y, z [, r, g, b])` | Spawn a `ShapeRegistry` primitive (`"cube"`, `"sphere"`, …) with optional tint |
| `despawn_self()` | Despawn the scripted entity |
| `despawn_by_prefix(prefix)` | Despawn every entity whose `Name` starts with `prefix` |
| `load_scene(path)` | Load a scene by path |

## Visibility, material & debug draw

| Function | Description |
|----------|-------------|
| `set_visibility(visible)` | Show / hide this entity |
| `set_material_color(r, g, b [, a])` | Set the base color (0..1 floats) |
| `screen_shake(intensity, duration)` | Trigger a camera shake |
| `draw_line(sx, sy, sz, ex, ey, ez [, duration])` | Draw a red debug line **in the 3D scene** |
| `print_log(msg)` | Write to the engine console at Info level |
| `print(...)` | Standard-library print |

`draw_line` here is a world-space debug line and is unrelated to `g.line` in [`on_draw`](#on_draw--the-2d-canvas), which is 2D screen drawing on a canvas.

## Cursor & environment

| Function | Description |
|----------|-------------|
| `lock_cursor()` | Grab and hide the cursor |
| `unlock_cursor()` | Release the cursor |
| `set_sun_angles(azimuth, elevation)` | Position the sun (degrees) |
| `set_fog(enabled, start, end)` | Toggle and range distance fog |
| `set_wind(speed, direction)` | Speed in m/s, direction in degrees the wind travels *toward* |
| `set_wind_gusts(strength, frequency, turbulence)` | Gust shaping |

## Timers

| Function | Description |
|----------|-------------|
| `start_timer(name, duration [, repeat])` | Start a timer; finished names appear in `timers_finished` |
| `stop_timer(name)` | Cancel a timer |

## Networking

Native only. See [Multiplayer](/docs/r1-alpha7/multiplayer/overview).

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

Requests are asynchronous (native only); responses arrive at `on_http` on a later frame, tagged by the callback name.

> **Needs a network backend plugin** (`plugins/http`). The engine has no HTTP client of its own — see [Network backends](../extending/network-backends.md). With none present a request answers immediately with `status == 0` and an error body, rather than hanging.

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

An HTTP error status is **not** a failure here: a 404 arrives at `on_http` with `status == 404` and whatever body the server sent, which is how you read an API's own error message. Only `status == 0` means the request never reached a server, and `body` is then the transport error.

## Assets

| Function | Description |
|----------|-------------|
| `asset_progress()` | Returns a table `{ state, total_files, loaded_files, total_bytes, loaded_bytes, fraction, current_path, elapsed_secs }`, or `nil` when idle |
| `is_loading()` | Convenience: `state == "loading"` |
| `is_loaded()` | Convenience: `state == "done"` |
| `scene_load_state()` | Returns `{ phase, current_path, progress }`, or `nil` before any scene load has been observed. `phase` is `"idle"` / `"loading"` / `"ready"` / `"failed"` |

> **These two measure different things and a loading screen usually needs both.** `scene_load_state()` tracks the *scene* — parsed off-thread, spawned across frames by the streamer, reaching `ready` when the last entity lands. `asset_progress()` tracks how many of that scene's **models** have finished loading, which keeps running after the scene is fully spawned. Wait only on `scene_load_state()` and you will uncover a world whose meshes are still popping in.
>
> Two limits on `asset_progress()` worth knowing before you promise a percentage: it counts glTF model loads (via `PendingMeshInstanceRehydrate`), not textures, audio or materials outside that path; and `total_bytes`/`loaded_bytes` come from the rpak index, so they are **zero in the editor or a `--project` run** — fall back to the file-count ratio there.

## Events

| Function | Description |
|----------|-------------|
| `emit(name, args)` | Broadcast an event; every script's `on_event(name, args)` fires next frame, as do Rust observers of `renzora::GameEvent` |

Delivery is deferred by one frame, so a script never sees its own emit within the same hook — dispatching inline would re-enter the VM mid-call and let a handler that emits recurse unbounded. An event with no listeners is a normal outcome, unlike an unclaimed `action()` name.

An event is the right shape when the sender should not have to know who cares. `set_on("music", …)` is right when you know exactly what you are talking to; "the boss died" may interest a quest tracker, an achievement check and a save trigger, and the boss should not have to know that any of them exist.

From Rust, listen with an observer:

```rust
app.add_observer(|trigger: On<renzora::GameEvent>| {
    if trigger.event().name == "boss_died" { /* … */ }
});
```

## Math helpers

`vec2`/`vec3` return a table (`{ x, y }` / `{ x, y, z }`).

| Function | Description |
|----------|-------------|
| `vec2(x, y)` | Construct a 2D vector table |
| `vec3(x, y, z)` | Construct a 3D vector table |
| `lerp(a, b, t)` | Linear interpolation |
| `clamp(v, min, max)` | Constrain to range |

## The action() catalog

`action(name, args)` fires a generic `ScriptAction` event observed by domain crates — the escape hatch for verbs with no dedicated function. `action_on(target, name, args)` targets a named entity.

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
| Parkour (`renzora_parkour`) | `parkour_move`, `parkour_sprint`, `parkour_jump`, `parkour_action`, `parkour_release` |
| Navmesh (`renzora_navmesh`) | `nav_set_destination`, `nav_clear_destination` |
| Wind (`renzora_wind`) | `set_wind`, `set_wind_gusts` |
| Animation (`renzora_animation`) | `set_anim_param`, `set_anim_bool`, `set_anim_trigger` |
| Ragdoll (`renzora_ragdoll`) | `enable_ragdoll`, `disable_ragdoll` |
| Camera (`renzora_engine`) | `set_fov` |

Tweens also run through `action()`: `tween_position`, `tween_rotation` (Euler degrees), `tween_scale`. Easing defaults to `ease_in_out` if the name is unrecognized.

> For widget *data* (a slider value, a bar fill), prefer reflection: `set_on("VolumeSlider", "SliderData.value", 0.5)`. There is **no** cross-scene variable store: `global_set` / `global_get` were removed along with the lifecycle graph that backed them, and a replacement is being designed.

## How domain functions get declared

The functions in the Physics, Parkour, Navigation, Animation and Wind sections are not built into the interpreter. Each domain crate *declares* them through the `ScriptExtension` trait, and every language backend builds them from that declaration:

```rust
impl ScriptExtension for MyScriptExtension {
    fn name(&self) -> &str { "combat" }
    fn bindings(&self) -> Vec<Binding> {
        vec![Bind::action("deal_damage", "deal_damage")
            .arg("amount", ParamKind::Float)
            .build()]
    }
}
```

The trait is purely declarative — the crate says what a function is called and what arguments it takes, and links no interpreter at all. That is why adding a language does not mean re-implementing the domain vocabulary, and why a new domain function appears in every language at once.

Core, engine-wide primitives (`set_position`, `play_sound`, `spawn_entity`, the reflection `set`/`get`/`set_on`, …) live in the language plugin's own `register_api()` instead. See [Script API Bindings](../extending/script-bindings.md) for how to add one, and [Script Backends](../extending/script-backends.md) for how to add a language.

## Capabilities not exposed as functions

The `ScriptCommand` enum (`command.rs`) defines engine verbs that have **no named function binding**. They are reachable from text scripts only via `action()`/extensions, if at all — calling them by name will fail:

`apply_torque`, `set_angular_velocity`, `Raycast`, all particle ops (`particle_play`/`burst`/`set_rate`/…), health (`set_health`, `damage`, `heal`, `kill`, `revive`, `set_invincible`), `camera_follow` / `set_camera_target` / `set_camera_zoom`, `spawn_prefab` / `unload_scene`, debug draws (`draw_ray` / `draw_sphere` / `draw_box` / `draw_point`), and `set_light_intensity` / `set_light_color`.

> Do not document these as available globals — an old API draft invented names such as `rpc_send`, `is_server`, `get_network_id`, `raycast_down`, `find_entity_by_name`, and `terrain_get_height` that **do not exist** in the engine.

## Rust scripts

A `<project>/scripts/*.rs` file is not this API at all. It is compiled into a [native plugin](../extending/native-plugins.md) and called once per frame per entity with `&mut World` — full Bevy, no command vocabulary, no queue.

`RustScriptBackend` claims the `.rs` extension so the Scripts component accepts one and the execution loop does not flag it as broken, but it returns no `ScriptCommand`s, because there genuinely are none: the whole reason to write Rust is the `&mut World` that no command vocabulary can stand in for. Gated on play mode exactly like Lua; recompiles on save, off the main thread. See [Rust Scripts](../scripting/rust-scripts.md).

## Blueprints

Visual [Blueprints](/docs/r1-alpha7/scripting/blueprints) (`.blueprint` / `.bp`, JSON-serialized `BlueprintGraph`) **compile to Lua** and run through the script VM. There is no live graph interpreter — compilation is the single execution path, the way Unreal compiles its Blueprints to bytecode, so a blueprint and a hand-written script are the same thing by the time they execute.

Two consequences worth knowing. A blueprint reaches the engine through exactly the vocabulary on this page, so anything outside the node palette has to be written in Lua. And a blueprint **needs the Lua backend present**: remove `plugins/lua` and blueprints stop running too.

Blueprints do expose collision, timer, and message *events* (`event/on_collision_enter`, `event/on_timer`, `event/on_message`, …) that text scripts have no hook for.

## Porting from Rhai

`.rhai` scripts no longer run. The function names carry over almost unchanged — every Rhai function was a subset of the Lua surface — so porting is mostly syntax:

| Feature | Rhai | Lua |
|---------|------|-----|
| Local variable | `let x = 5` | `local x = 5` |
| Map / table | `#{ key: value }` | `{ key = value }` |
| Nil / empty | `()` | `nil` |
| String concat | `+` | `..` |
| Not equal | `!=` | `~=` |
| Array index | 0-based | 1-based |
| Block end | `}` | `end` |
| Logical ops | `&&` / `\|\|` / `!` | `and` / `or` / `not` |

Two renames to watch for: Rhai's `play_sound_at_volume(path, volume)` is Lua's `play_sound(path, volume)`, and `start_timer_repeat(name, duration)` is `start_timer(name, duration, true)`. Rhai's key/gamepad helpers took a leading map or array argument (`is_key_pressed(keys, "Space")`); the Lua ones do not (`is_key_pressed("Space")`).

Everything Rhai could not do — `action()`, networking, HTTP, bulk reflection, `on_draw`, and every hook past `on_update` — is available once ported.

## See also

- [Lua](/docs/r1-alpha7/scripting/lua) — guided introduction to the backend
- [Rust Scripts](/docs/r1-alpha7/scripting/rust-scripts) — `&mut World` per entity
- [Visual Blueprints](/docs/r1-alpha7/scripting/blueprints) — node graphs interpreted at runtime
- [Input Handling](/docs/r1-alpha7/scripting/input) — the action map and key names
- [Game UI](/docs/r1-alpha7/scripting/game-ui) — markup, `ui_*` verbs, and bindings
- [Script API Bindings](/docs/r1-alpha7/extending/script-bindings) — declaring a new function from a domain crate
- [Script Backends](/docs/r1-alpha7/extending/script-backends) — adding a language
