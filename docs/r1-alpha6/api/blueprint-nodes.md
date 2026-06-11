# Blueprint Node API

Every built-in blueprint node, its `node_type` string, and its pins — generated from the `ALL_NODES` registry in `renzora_blueprint`.

This is the per-node reference for [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints). For the data model (pins, wires, the `.blueprint` file format) start there; to register node types of your own see [Custom Blueprint Nodes](/docs/r1-alpha5/extending/custom-nodes).

## How to read these tables

Node-type strings are namespaced `category/name` (for example `transform/set_position`). Every node also has a friendly display name shown in the editor palette.

Pins are either **execution** pins (the white flow wires, `PinType::Exec`) or **data** pins (typed values). To keep the tables readable:

- **Action nodes** (the ones that change the world) follow a standard convention: one `exec` **input** and one `then` exec **output**. Those two are *not* repeated in every row — only **data** pins are listed, plus any *non-standard* exec pins (called out in the Notes column, e.g. Branch's `true`/`false`).
- **Pure data nodes** (Math, getters, queries) have **no exec pins at all** — they only run when a downstream node pulls their output.
- **Event nodes** have an `exec` **output** and no exec input; they are the graph's entry points.

Inline defaults are shown as `= value`. Data pins use this `PinType` set:

| Pin type | Meaning |
|----------|---------|
| `Exec` | Execution flow (not a value) |
| `Float` | 32-bit float |
| `Int` | 32-bit signed integer |
| `Bool` | `true` / `false` |
| `String` | UTF-8 text |
| `Vec2` | 2-component vector |
| `Vec3` | 3-component vector |
| `Color` | RGBA (4 floats) |
| `Entity` | entity reference (resolved by name at runtime) |
| `Any` | wildcard — accepts any non-exec type |

## Event

Entry points. Each has an `exec` output and no exec input; the interpreter starts a flow from it when its trigger fires.

| Node | `node_type` | Data outputs | Fires when |
|------|-------------|--------------|------------|
| On Ready | `event/on_ready` | — | The entity is first initialized (once per play session; re-fires when play mode restarts) |
| On Update | `event/on_update` | `delta` `Float`, `elapsed` `Float` | Every frame |
| On Collision Enter | `event/on_collision_enter` | `other` `Entity` | This entity starts colliding with another |
| On Collision Exit | `event/on_collision_exit` | `other` `Entity` | A collision ends |
| On Timer | `event/on_timer` | — (input `timer_name` `String` = `"my_timer"`) | A named timer completes |
| On Message | `event/on_message` | — (input `message` `String` = `"my_message"`) | A named message arrives (UI, scripts, or other blueprints) |

Three more entry-style nodes live in other categories (they also have an `exec` output and no exec input): [`animation/on_finished`](#animation), [`network/on_message`](#network), and [`lifecycle/on_scene_loaded`](#lifecycle).

> **Runtime status.** The live interpreter (`interpreter::run_blueprints`) currently dispatches **On Ready** and **On Update**. The collision, timer, message, network-message, on-scene-loaded, and on-animation-finished entry nodes exist in the palette and serialize into the file format, but their firing is still being wired into the interpreter (`// TODO: on_collision, on_timer, on_message`) — don't rely on them triggering in a shipped game yet.

## Flow

| Node | `node_type` | Data inputs | Notes |
|------|-------------|-------------|-------|
| Branch | `flow/branch` | `condition` `Bool` = `true` | Exec out is `true` / `false` (not `then`) — if/else routing |
| Sequence | `flow/sequence` | — | Exec outs `then_0`, `then_1`, `then_2` run in order |
| Do Once | `flow/do_once` | — | Extra exec **input** `reset`; exec out is `completed`. Passes once until reset |
| Flip Flop | `flow/flip_flop` | — | Exec outs `a` / `b` alternate; data out `is_a` `Bool` |
| Gate | `flow/gate` | `start_open` `Bool` = `true` | Extra exec inputs `open` / `close` / `toggle`; exec out is `exit` |
| Delay | `flow/delay` | `duration` `Float` = `1.0` | Exec out is `completed` |
| Counter | `flow/counter` | `step` `Float` = `1.0`, `min` `Float` = `0.0`, `max` `Float` = `1.0`, `loop` `Bool` = `true` | Data out `value` `Float`; increments each run, wraps when `loop` |
| Start Timer | `flow/start_timer` | `name` `String` = `"my_timer"`, `duration` `Float` = `1.0`, `repeat` `Bool` = `false` | Standard `exec` → `then` |

> There is **no For Loop, While Loop, or Switch node**. Iterate with `flow/counter` plus event re-entry; select with `flow/branch`.

## Math

All Math nodes are pure data (no exec pins).

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Add | `math/add` | `a` `Float` = `0`, `b` `Float` = `0` | `result` `Float` |
| Subtract | `math/subtract` | `a` `Float` = `0`, `b` `Float` = `0` | `result` `Float` |
| Multiply | `math/multiply` | `a` `Float` = `1`, `b` `Float` = `1` | `result` `Float` |
| Divide | `math/divide` | `a` `Float` = `1`, `b` `Float` = `1` | `result` `Float` (returns `0` if `b == 0`) |
| Negate | `math/negate` | `value` `Float` | `result` `Float` |
| Abs | `math/abs` | `value` `Float` | `result` `Float` |
| Clamp | `math/clamp` | `value` `Float` = `0.5`, `min` `Float` = `0`, `max` `Float` = `1` | `result` `Float` |
| Lerp | `math/lerp` | `a` `Float` = `0`, `b` `Float` = `1`, `t` `Float` = `0.5` | `result` `Float` |
| Random Range | `math/random_range` | `min` `Float` = `0`, `max` `Float` = `1` | `result` `Float` |
| Sin | `math/sin` | `value` `Float` | `result` `Float` |
| Cos | `math/cos` | `value` `Float` | `result` `Float` |
| Compare | `math/compare` | `a` `Float` = `0`, `b` `Float` = `0` | `greater` `Bool` (A&nbsp;>&nbsp;B), `less` `Bool` (A&nbsp;<&nbsp;B), `equal` `Bool` (A&nbsp;==&nbsp;B) |
| AND | `math/and` | `a` `Bool`, `b` `Bool` | `result` `Bool` |
| OR | `math/or` | `a` `Bool`, `b` `Bool` | `result` `Bool` |
| NOT | `math/not` | `value` `Bool` | `result` `Bool` |
| Combine Vec3 | `math/combine_vec3` | `x` `Float`, `y` `Float`, `z` `Float` | `result` `Vec3` |
| Split Vec3 | `math/split_vec3` | `vector` `Vec3` | `x` `Float`, `y` `Float`, `z` `Float` |
| Min | `math/min` | `a` `Float`, `b` `Float` | `result` `Float` |
| Max | `math/max` | `a` `Float`, `b` `Float` | `result` `Float` |
| Floor | `math/floor` | `value` `Float` | `result` `Float` |
| Ceil | `math/ceil` | `value` `Float` | `result` `Float` |
| Round | `math/round` | `value` `Float` | `result` `Float` |
| Modulo | `math/modulo` | `a` `Float` = `0`, `b` `Float` = `1` | `result` `Float` |
| Distance | `math/distance` | `a` `Vec3`, `b` `Vec3` | `distance` `Float` |
| Dot Product | `math/dot` | `a` `Vec3`, `b` `Vec3` | `result` `Float` |
| Cross Product | `math/cross` | `a` `Vec3`, `b` `Vec3` | `result` `Vec3` |
| Normalize | `math/normalize` | `value` `Vec3` | `result` `Vec3` |

## String

Pure data nodes for text.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Concat | `string/concat` | `a` `String`, `b` `String` | `result` `String` |
| Format | `string/format` | `template` `String` = `"Value: {0}"`, `value` `Any` | `result` `String` (replaces `{0}`) |
| String to Float | `string/to_float` | `value` `String` = `"0"` | `result` `Float` |
| String to Int | `string/to_int` | `value` `String` = `"0"` | `result` `Int` |

## Convert

Pure data type conversions.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| To String | `convert/to_string` | `value` `Any` | `result` `String` |
| To Float | `convert/to_float` | `value` `Any` | `result` `Float` |
| To Int | `convert/to_int` | `value` `Any` | `result` `Int` |
| To Bool | `convert/to_bool` | `value` `Any` | `result` `Bool` |

## Transform

Operates on **this entity's** transform. Getters are pure data; setters use the standard `exec` → `then` flow.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Get Position | `transform/get_position` | — | `position` `Vec3`, `x` `Float`, `y` `Float`, `z` `Float` |
| Set Position | `transform/set_position` | `position` `Vec3` | — |
| Translate | `transform/translate` | `offset` `Vec3` | — (moves by offset) |
| Get Rotation | `transform/get_rotation` | — | `rotation` `Vec3` (euler degrees), `x`, `y`, `z` `Float` |
| Set Rotation | `transform/set_rotation` | `rotation` `Vec3` (euler degrees) | — |
| Rotate | `transform/rotate` | `degrees` `Vec3` | — (rotates by degrees) |
| Look At | `transform/look_at` | `target` `Vec3` | — (faces the target position) |
| Set Scale | `transform/set_scale` | `scale` `Vec3` = `(1, 1, 1)` | — |
| Get Forward | `transform/get_forward` | — | `forward` `Vec3`, `right` `Vec3`, `up` `Vec3` |

## Input

All Input nodes are pure data — sample them from a flow driven by `event/on_update`.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Get Movement | `input/get_movement` | — | `movement` `Vec2` (normalized WASD/arrows), `x` `Float`, `y` `Float` |
| Is Key Pressed | `input/is_key_pressed` | `key` `String` = `"Space"` | `pressed` `Bool` (held down) |
| Is Key Just Pressed | `input/is_key_just_pressed` | `key` `String` = `"Space"` | `pressed` `Bool` (this frame only) |
| Get Mouse Position | `input/get_mouse_position` | — | `position` `Vec2`, `delta` `Vec2` |
| Is Mouse Pressed | `input/is_mouse_pressed` | `button` `Int` = `0` (0=left, 1=right, 2=middle) | `pressed` `Bool` |
| Get Gamepad | `input/get_gamepad` | — | `left_stick` `Vec2`, `right_stick` `Vec2`, `left_trigger` `Float`, `right_trigger` `Float` |
| Is Action Pressed | `input/is_action_pressed` | `action` `String` = `"jump"` | `pressed` `Bool` |
| Is Action Just Pressed | `input/is_action_just_pressed` | `action` `String` = `"jump"` | `pressed` `Bool` |
| Get Action Axis | `input/get_action_axis` | `action` `String` = `"move"` | `value` `Float` (-1 to 1) |
| Get Action Axis 2D | `input/get_action_axis2d` | `action` `String` = `"move"` | `value` `Vec2`, `x` `Float`, `y` `Float` |

## Entity

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Get Self | `entity/get_self` | — | `entity` `Entity` (this entity), `name` `String` |
| Get Entity | `entity/get_entity` | `name` `String` | `entity` `Entity`, `found` `Bool` |
| Spawn Entity | `entity/spawn` | `name` `String` = `"New Entity"` | `entity` `Entity` — standard `exec` → `then` |
| Despawn Entity | `entity/despawn` | `entity` `Entity` | — standard `exec` → `then` |
| Despawn Self | `entity/despawn_self` | — | `exec` input only (destroys this entity) |

## Component

Reflection-based access to any registered component. The default `component`/`field` pins target `Transform.translation`.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Get Component Field | `component/get_field` | `entity` `Entity`, `component` `String` = `"Transform"`, `field` `String` = `"translation"` | `value` `Any` |
| Set Component Field | `component/set_field` | `entity` `Entity`, `component` `String` = `"Transform"`, `field` `String` = `"translation"`, `value` `Any` | — standard `exec` → `then` |

## Physics

Acts on **this entity's** rigidbody. Getters/queries are pure data; the rest use `exec` → `then`.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Apply Force | `physics/apply_force` | `force` `Vec3` | — (continuous force) |
| Apply Impulse | `physics/apply_impulse` | `impulse` `Vec3` = `(0, 10, 0)` | — (instant impulse) |
| Set Velocity | `physics/set_velocity` | `velocity` `Vec3` | — (sets linear velocity) |
| Kinematic Slide | `physics/kinematic_slide` | `delta` `Vec3` | — (collide-and-slide move) |
| Raycast | `physics/raycast` | `origin` `Vec3`, `direction` `Vec3` = `(0, -1, 0)`, `max_distance` `Float` = `100` | `point` `Vec3`, `normal` `Vec3`, `entity` `Entity`, `distance` `Float`. Exec outs are `hit` / `miss` (not `then`) |
| Is Grounded | `physics/is_grounded` | — | `grounded` `Bool` |
| Get Velocity | `physics/get_velocity` | — | `velocity` `Vec3`, `speed` `Float` |

## Audio

All Audio nodes use `exec` → `then`.

| Node | `node_type` | Inputs |
|------|-------------|--------|
| Play Sound | `audio/play_sound` | `path` `String` = `"sounds/click.ogg"`, `volume` `Float` = `1.0`, `looping` `Bool` = `false` |
| Play Music | `audio/play_music` | `path` `String` = `"music/theme.ogg"`, `volume` `Float` = `0.8`, `fade_in` `Float` = `1.0` |
| Stop Music | `audio/stop_music` | `fade_out` `Float` = `1.0` |

> Audio is **native-only** (Kira). On the WASM/web export these nodes are no-ops.

## UI

Drive game UI widgets by name. All use `exec` → `then`.

| Node | `node_type` | Inputs |
|------|-------------|--------|
| Show UI | `ui/show` | `path` `String` = `"ui/main_menu.ui"` |
| Hide UI | `ui/hide` | `path` `String` = `"ui/main_menu.ui"` |
| Toggle UI | `ui/toggle` | `name` `String` (canvas name) |
| Set UI Text | `ui/set_text` | `element` `String`, `text` `String` |
| Set UI Progress | `ui/set_progress` | `element` `String`, `value` `Float` = `1.0` (0–1) |
| Set UI Health | `ui/set_health` | `element` `String`, `current` `Float` = `75`, `max` `Float` = `100` |
| Set UI Slider | `ui/set_slider` | `element` `String`, `value` `Float` = `0.5` |
| Set UI Checkbox | `ui/set_checkbox` | `element` `String`, `checked` `Bool` = `true` |
| Set UI Toggle | `ui/set_toggle` | `element` `String`, `on` `Bool` = `true` |
| Set UI Visible | `ui/set_visible` | `element` `String` (empty = self), `visible` `Bool` = `true` |
| Set UI Theme | `ui/set_theme` | `theme` `String` = `"dark"` |
| Set UI Color | `ui/set_color` | `element` `String`, `color` `Color` = `(1, 1, 1, 1)` |

## Scene

| Node | `node_type` | Inputs |
|------|-------------|--------|
| Load Scene | `scene/load` | `path` `String` = `"scenes/main.ron"` — standard `exec` → `then` |

## Variable

Per-instance graph variables, stored in the interpreter's runtime state and reset when play mode restarts.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Get Variable | `variable/get` | `name` `String` = `"my_var"` | `value` `Any` |
| Set Variable | `variable/set` | `name` `String` = `"my_var"`, `value` `Any` | — standard `exec` → `then` |

## Rendering

| Node | `node_type` | Inputs |
|------|-------------|--------|
| Set Visibility | `rendering/set_visibility` | `visible` `Bool` = `true` |
| Set Material Color | `rendering/set_material_color` | `color` `Color` = `(1, 1, 1, 1)` (base color of this entity's material) |

Both use `exec` → `then`. Material colour lives here, not in a separate "Material" category — there is no Set Emissive, Set Material Property, or Swap Material node.

## Animation

Acts on **this entity's** animator. Setters use `exec` → `then`; reads are pure data; `animation/on_finished` is an entry node.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Play Animation | `animation/play` | `name` `String`, `looping` `Bool` = `true`, `speed` `Float` = `1.0` | — |
| Stop Animation | `animation/stop` | — | — |
| Pause Animation | `animation/pause` | — | — |
| Resume Animation | `animation/resume` | — | — |
| Set Animation Speed | `animation/set_speed` | `speed` `Float` = `1.0` | — |
| Crossfade Animation | `animation/crossfade` | `name` `String`, `duration` `Float` = `0.3`, `looping` `Bool` = `true` | — |
| Set Anim Param | `animation/set_param` | `name` `String` (param), `value` `Float` = `0.0` | — (state-machine float param) |
| Set Anim Bool | `animation/set_bool_param` | `name` `String` (param), `value` `Bool` = `false` | — (state-machine bool param) |
| Trigger Anim | `animation/trigger` | `name` `String` (trigger) | — (one-shot state-machine trigger) |
| Set Layer Weight | `animation/set_layer_weight` | `layer` `String`, `weight` `Float` = `1.0` | — |
| Tween Position | `animation/tween_position` | `target` `Vec3`, `duration` `Float` = `1.0`, `easing` `String` = `"ease_in_out"` | — |
| Get Animation Time | `animation/get_time` | — | `time` `Float` (pure data) |
| Get Animation Length | `animation/get_length` | `name` `String` | `length` `Float` (seconds, 0 if not loaded; pure data) |
| Get Anim Param | `animation/get_param` | `name` `String` | `value` `Float` (pure data) |
| Get Anim Bool | `animation/get_bool` | `name` `String` | `value` `Bool` (pure data) |
| Is Animation Playing | `animation/is_playing` | — | `playing` `Bool` (pure data) |
| On Animation Finished | `animation/on_finished` | — | **Event node:** `exec` output + `name` `String` (clip name). Fires when a non-looping clip finishes |

## Network

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Is Server | `network/is_server` | — | `value` `Bool` (pure data) |
| Is Connected | `network/is_connected` | — | `value` `Bool` (pure data) |
| Send Message | `network/send_message` | `channel` `String` = `"default"`, `data` `String` | — standard `exec` → `then` |
| Net Spawn | `network/spawn` | `name` `String`, `position` `Vec3` | — standard `exec` → `then` |
| On Message | `network/on_message` | `channel` `String` = `"default"` | **Event node:** `exec` output + `data` `String` + `sender` `Int` (sender ID) |

> **Network nodes are minimal today.** The blueprint interpreter does not read the networking crate, so `Is Server` / `Is Connected` evaluate to `false`, and `Send Message` / `Net Spawn` map onto the same TODO/stub network actions as the scripting API. Treat the Network category as forward-looking. See [Multiplayer](/docs/r1-alpha5/multiplayer) for the real status.

## Lifecycle

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| On Scene Loaded | `lifecycle/on_scene_loaded` | — | **Event node:** `exec` output + `scene` `String` (loaded scene path) |
| Global Get | `lifecycle/global_get` | `key` `String` | `value` `String` (reads the cross-system global store) |
| Global Set | `lifecycle/global_set` | `key` `String`, `value` `String` | — standard `exec` → `then` |

`global_get` / `global_set` share the same global store as the scripting `global_get` / `global_set` actions, so blueprints and Lua/Rhai scripts can exchange values.

## Navigation

Drives this entity's `NavAgent` (requires the navmesh subsystem). Queries are pure data.

| Node | `node_type` | Inputs | Outputs |
|------|-------------|--------|---------|
| Set Destination | `navigation/set_destination` | `target` `Vec3` | — standard `exec` → `then` |
| Clear Destination | `navigation/clear_destination` | — | — standard `exec` → `then` (stops the agent) |
| Has Path | `navigation/has_path` | — | `has_path` `Bool` |
| Has Target | `navigation/has_target` | — | `has_target` `Bool` |
| Is At Destination | `navigation/is_at_destination` | — | `arrived` `Bool` |
| Distance To Destination | `navigation/distance_to_destination` | — | `distance` `Float` (planar XZ) |

## Debug

| Node | `node_type` | Inputs |
|------|-------------|--------|
| Log | `debug/log` | `message` `String` = `"Hello!"` (prints to the console) |
| Draw Line | `debug/draw_line` | `start` `Vec3`, `end` `Vec3`, `color` `Color` = `(1, 0, 0, 1)`, `duration` `Float` = `0.0` |

Both use `exec` → `then`.

## Looking up nodes in code

The registry is a flat slice you can iterate from Rust:

```rust
use renzora_blueprint::{ALL_NODES, categories, node_def, nodes_in_category};

// Every node definition.
for def in ALL_NODES {
    println!("{} ({})", def.display_name, def.node_type);
}

// Categories in editor display order.
let cats: Vec<&str> = categories();

// Nodes in one category, or a single definition by type.
let physics = nodes_in_category("Physics");
let set_pos = node_def("transform/set_position").unwrap();
```

Each entry is a `BlueprintNodeDef { node_type, display_name, category, description, pins, color }`, where `pins` is a function returning the node's `PinTemplate`s. This is the same data the editor palette and the interpreter dispatch on, so adding a node in one place keeps both in sync — see [Custom Blueprint Nodes](/docs/r1-alpha5/extending/custom-nodes).

## See also

- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — concepts, pins/wires, and the `.blueprint` file format
- [Custom Blueprint Nodes](/docs/r1-alpha5/extending/custom-nodes) — register your own node types
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how blueprints and Lua/Rhai scripts share the same downstream actions
