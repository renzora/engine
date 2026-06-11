# Visual Blueprints

Wire up entity logic with a node graph instead of code — saved as a `.blueprint` file and interpreted directly by the engine at runtime.

## What a blueprint is

A blueprint is a visual node graph stored as a `BlueprintGraph`. On disk it is a `.blueprint` file (the `.bp` extension is an accepted alias) containing **JSON** — a list of nodes and the wires (connections) between their pins. The same graph type is also a regular ECS **component**, so blueprints serialize straight into a scene's `.ron` alongside everything else on the entity.

The system lives in the `renzora_blueprint` crate. Its `BlueprintPlugin` registers itself with `renzora::add!(BlueprintPlugin)` at runtime scope, so blueprints run in **both the editor (play mode) and exported games** — there is nothing extra to enable.

Every frame that scripts are running, the interpreter (`interpreter::run_blueprints`) walks each entity that has a `BlueprintGraph`, starting from its event nodes, and emits engine actions — `ScriptAction`s, transform writes, and character commands — that the same physics/audio/UI/animation systems consume from Lua and Rhai scripts.

> Blueprints are **interpreted directly** — they are *not* compiled to Lua (or to any bytecode) before they run. The graph is walked live each tick. The old claim that "blueprints compile to the same internal representation as scripts with no performance difference" is wrong: they are a distinct, interpreted execution path.

### Blueprints vs. text scripts

Blueprints and text scripts ([Lua](/docs/r1-alpha5/scripting/lua) / [Rhai](/docs/r1-alpha5/scripting/rhai)) are **separate systems** that happen to share the same downstream action plumbing:

- A blueprint is **not** a 1:1 visual mirror of the scripting API. It exposes its own, smaller node palette (listed below). Anything outside that palette has to be done in Lua or Rhai.
- A single entity can carry **both** a `BlueprintGraph` component and a `ScriptComponent` — they run side by side and write to the same world.
- Blueprint event nodes mirror only a subset of the script lifecycle hooks; there is no blueprint equivalent of, for example, `on_rpc` or `on_http`.

> **Editor-only "bake to Lua".** The editor can export a graph to a Lua file (`apply_blueprint_to_lua` → `scripts/bp_<name>.lua`) and attach it as a script. This is a one-way convenience for reading/extending the generated code — it is **not** how blueprints normally execute. The shipped runtime always interprets the `BlueprintGraph` directly.

## The Blueprint Editor

Blueprints are edited in the **Blueprints** workspace (one of the editor's ribbon workspaces) using the **Blueprint Editor** panel. It works in two modes:

- **Scene mode** (default) — the editor edits the `BlueprintGraph` **component on the currently selected entity**. The graph follows your selection and is saved as part of the scene.
- **Asset mode** — a standalone `.blueprint` file is open in a document tab; edits are written back to that file. Open one by double-clicking a `.blueprint` in the Assets browser.

To create a new blueprint, use the Assets browser's **New → Blueprint** entry (it creates `NewBlueprint.blueprint`), then either open it in Asset mode or add a `BlueprintGraph` to an entity and author it in Scene mode.

## Pins and wires

Every node has typed **pins**. Connections come in two flavours:

- **Execution pins** (`Exec`) — the white "flow" wires. An output exec pin can fan out to several targets; flow runs left to right from an event node.
- **Data pins** — carry a value. A data input accepts exactly one wire; if nothing is connected it falls back to the node's inline constant, then to the pin's default.

Data pins use the `PinType` enum:

| Pin type | Notes |
|----------|-------|
| `Exec` | Execution flow (not a value) |
| `Float` | 32-bit float |
| `Int` | 32-bit signed integer |
| `Bool` | true / false |
| `String` | UTF-8 text |
| `Vec2` | 2-component vector |
| `Vec3` | 3-component vector |
| `Color` | RGBA (4 floats) |
| `Entity` | reference to an entity (resolved by name at runtime) |
| `Any` | wildcard — accepts any non-exec type |

The editor allows these **implicit conversions** when wiring mismatched data pins (`PinType::compatible`):

- `Int → Float`
- `Float → Vec2 / Vec3 / Color`
- `Vec3 ↔ Color`
- `Bool → Int / Float`
- any non-exec type ↔ `Any`

## Node categories

The palette is organised into these categories (in editor display order). Node-type strings are namespaced (`category/name`, e.g. `transform/set_position`).

| Category | What's in it (examples) |
|----------|-------------------------|
| **Event** | Entry points — `on_ready`, `on_update`, `on_collision_enter` |
| **Flow** | `branch`, `sequence`, `do_once`, `flip_flop`, `gate`, `delay`, `counter`, `start_timer` |
| **Math** | `add`/`subtract`/`multiply`/`divide`, `clamp`, `lerp`, `sin`/`cos`, `min`/`max`, `distance`, `dot`, `cross`, `normalize`, `combine_vec3`/`split_vec3` |
| **String** | `concat`, `format`, `to_float`, `to_int` |
| **Convert** | `to_string`, `to_float`, `to_int`, `to_bool` |
| **Transform** | `get_position`/`set_position`, `translate`, `get_rotation`/`set_rotation`, `rotate`, `look_at`, `set_scale`, `get_forward` |
| **Input** | `get_movement`, `is_key_pressed`, `is_key_just_pressed`, `get_mouse_position`, `is_mouse_pressed`, `get_gamepad`, `is_action_pressed`, `get_action_axis`/`get_action_axis2d` |
| **Entity** | `get_self`, `get_entity`, `spawn`, `despawn`, `despawn_self` |
| **Component** | `get_field`, `set_field` (reflection-based, any registered component) |
| **Physics** | `apply_force`, `apply_impulse`, `set_velocity`, `raycast`, `kinematic_slide`, `is_grounded`, `get_velocity` |
| **Audio** | `play_sound`, `play_music`, `stop_music` |
| **UI** | `show`/`hide`/`toggle`, `set_text`, `set_progress`, `set_health`, `set_slider`, `set_checkbox`, `set_toggle`, `set_visible`, `set_theme`, `set_color` |
| **Scene** | `load` |
| **Variable** | `get`, `set` (per-instance graph variables) |
| **Rendering** | `set_visibility`, `set_material_color` |
| **Animation** | `play`, `crossfade`, `stop`/`pause`/`resume`, `set_speed`, `set_param`/`set_bool_param`/`trigger`, `set_layer_weight`, `tween_position`, plus reads (`get_time`, `get_length`, `is_playing`) |
| **Network** | `is_server`, `is_connected`, `send_message`, `spawn` |
| **Lifecycle** | `on_scene_loaded`, `global_get`, `global_set` |
| **Navigation** | `set_destination`, `clear_destination`, `has_path`, `has_target`, `is_at_destination`, `distance_to_destination` |
| **Debug** | `log`, `draw_line` |

> Several blocks that other engines have do **not** exist here: there is no For Loop, While Loop, or Switch node. Iteration is done with `flow/counter` plus event re-entry, and selection with `flow/branch`.

The full per-node pin reference lives in the [Blueprint Node API](/docs/r1-alpha5/api/blueprint-nodes). To add your own node types, see [Custom Blueprint Nodes](/docs/r1-alpha5/extending/custom-nodes).

## Event nodes

Event nodes are the graph's entry points — they have an exec **output** but no exec input. The interpreter starts a flow at each of them when its trigger condition is met.

| Node type | Display name | Fires when | Outputs |
|-----------|--------------|------------|---------|
| `event/on_ready` | On Ready | The entity is first initialized (once per play session) | — |
| `event/on_update` | On Update | Every frame | `delta`, `elapsed` |
| `event/on_collision_enter` | On Collision Enter | This entity starts colliding with another | `other` (Entity) |
| `event/on_collision_exit` | On Collision Exit | A collision ends | `other` (Entity) |
| `event/on_timer` | On Timer | A named timer completes | — |
| `event/on_message` | On Message | A named message arrives (UI, scripts, other blueprints) | — |
| `animation/on_finished` | On Animation Finished | A non-looping clip finishes | `name` (Clip Name) |
| `network/on_message` | On Message | A named network message is received | `data`, `sender` (Sender ID) |
| `lifecycle/on_scene_loaded` | On Scene Loaded | A scene finishes loading | `scene` (path) |

> **Runtime status.** `event/on_ready` and `event/on_update` are the triggers driven by the runtime interpreter today; `on_ready` re-fires whenever play mode (re)starts. The remaining event nodes are present in the palette and the file format, but their live firing is still being wired into the interpreter — don't rely on them firing in a shipped game yet.

## Variables

Blueprint variables are read and written with the **Variable** nodes (`variable/get`, `variable/set`). They are stored per blueprint **instance** in the interpreter's runtime state and reset when play mode restarts, so a freshly started entity always begins from a clean slate. Reference a variable by its string name from any node in the graph.

## Data and execution flow

Two things happen as the interpreter walks a graph:

- **Execution** flows forward along exec wires (`exec`/`then`/`true`/`false`/...). Action nodes (Set Position, Play Sound, Apply Force, ...) emit their engine action when reached.
- **Data** is *pulled*: when an action node needs an input value, the interpreter evaluates the upstream data node feeding that pin (and caches the result for the rest of the tick). Pure data nodes (Math, Get Position, Is Grounded, ...) have no exec pins and only run when something downstream asks for their output.

## File format

A `.blueprint` is JSON. The top level is the `BlueprintGraph` (`nodes`, `connections`, `next_id`); each node carries its `id`, `node_type`, editor `position`, and any inline `input_values`; each connection links one node's output pin to another's input pin:

```json
{
  "nodes": [
    { "id": 1, "node_type": "event/on_update", "position": [0.0, 0.0], "input_values": {} },
    {
      "id": 2,
      "node_type": "transform/rotate",
      "position": [240.0, 0.0],
      "input_values": { "degrees": { "Vec3": [0.0, 90.0, 0.0] } }
    }
  ],
  "connections": [
    { "from_node": 1, "from_pin": "exec", "to_node": 2, "to_pin": "exec" }
  ],
  "next_id": 3
}
```

You normally never hand-edit this — the Blueprint Editor reads and writes it for you — but because it is plain JSON it diffs and merges in version control like any other text asset.

## See also

- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts and blueprints fit together
- [Blueprint Node API](/docs/r1-alpha5/api/blueprint-nodes) — every node and its pins
- [Custom Blueprint Nodes](/docs/r1-alpha5/extending/custom-nodes) — register your own node types
