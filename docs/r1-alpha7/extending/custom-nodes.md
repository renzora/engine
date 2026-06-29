# Custom Blueprint Nodes

Add your own node types to the visual blueprint system by extending the `renzora_blueprint` crate's static node registry and interpreter.

## How node types are defined

A blueprint node type is **not** a trait object you register at runtime. It is a pair of things compiled into the `renzora_blueprint` crate:

1. A **static declaration** — a `BlueprintNodeDef` value that describes the node's pins, category, label, and editor color. These live in `crates/renzora_blueprint/src/nodes.rs` and are collected into one slice, `ALL_NODES`.
2. A **behaviour arm** in the interpreter — `crates/renzora_blueprint/src/interpreter.rs` dispatches on the node's `node_type` string. You add a `match` arm there for your node.

The editor's node palette, the per-node property inspector, and the runtime interpreter all key off the same `node_type` string (e.g. `"transform/set_position"`), so the three pieces stay in sync as long as the string matches.

> There is no `register_blueprint_node`, no `BlueprintNode` execution trait, no `NodeContext`, and no `MockNodeContext`. Older drafts of this page described a plugin-style trait API that was never implemented. Adding a node means editing the `renzora_blueprint` workspace crate and rebuilding the engine — it is a fork-the-engine task, not a dlopen plugin hook. (`BlueprintNode` *is* a real type, but it is the serialized **node instance** in a graph, not a trait.)

The interpreter crate depends only on `renzora` (the contracts crate), never on `renzora_scripting`, so node behaviour is written against `renzora`'s shared types.

## The node definition

`renzora::BlueprintNodeDef` (re-exported as `renzora_blueprint::BlueprintNodeDef`) is a plain struct:

```rust
pub struct BlueprintNodeDef {
    pub node_type: &'static str,      // namespaced id, e.g. "math/square"
    pub display_name: &'static str,   // shown in the palette
    pub category: &'static str,       // groups it in the palette
    pub description: &'static str,    // tooltip text
    pub pins: fn() -> Vec<PinTemplate>,
    pub color: [u8; 3],               // RGB header color in the graph editor
}
```

Declare your node as a `pub static` and build its pins with the `PinTemplate` helpers:

```rust
use renzora_blueprint::{BlueprintNodeDef, PinTemplate, PinType, PinValue};

const CLR_MATH: [u8; 3] = [120, 120, 120];

pub static SQUARE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/square",
    display_name: "Square",
    category: "Math",
    description: "value * value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};
```

Then add a reference to it in the `ALL_NODES` slice at the bottom of `nodes.rs` (drop it in the section matching its category):

```rust
pub static ALL_NODES: &[&BlueprintNodeDef] = &[
    // ...
    &SQUARE,
    // ...
];
```

That single edit makes the node discoverable everywhere: `node_def("math/square")` resolves it, `nodes_in_category("Math")` includes it, and the editor palette lists it.

### Pin templates

Pins are built with four constructors plus an optional default. Execution pins carry no value; data pins do.

| Builder | Makes |
|---------|-------|
| `PinTemplate::exec_in(name, label)` | An execution **input** (the white flow wire entering the node) |
| `PinTemplate::exec_out(name, label)` | An execution **output** |
| `PinTemplate::input(name, label, PinType)` | A data **input** |
| `PinTemplate::output(name, label, PinType)` | A data **output** |
| `.with_default(PinValue)` | Sets the fallback value for a data input when nothing is wired |

A node with at least one `exec_in` is an **action node** (it runs when execution reaches it). A node with only data pins is a **pure node** (it is evaluated on demand when something downstream reads its output). Event nodes have an `exec_out` but no `exec_in`.

### Pin types and values

`PinType` declares the wire type; `PinValue` is the concrete value carried at runtime (and stored as a node's inline constant or a pin default).

| `PinType` | `PinValue` variant | Rust payload |
|-----------|--------------------|--------------|
| `Exec` | — | execution flow, no value |
| `Float` | `PinValue::Float` | `f32` |
| `Int` | `PinValue::Int` | `i32` |
| `Bool` | `PinValue::Bool` | `bool` |
| `String` | `PinValue::String` | `String` |
| `Vec2` | `PinValue::Vec2` | `[f32; 2]` |
| `Vec3` | `PinValue::Vec3` | `[f32; 3]` |
| `Color` | `PinValue::Color` | `[f32; 4]` (RGBA) |
| `Entity` | `PinValue::Entity` | `String` (entity resolved by **name** at runtime) |
| `Any` | `PinValue::None` (when empty) | wildcard — accepts any non-exec type |

> `Float` is `f32` and `Int` is `i32` — not 64-bit. An `Entity` pin carries a **name string**, not a live `Entity` handle; the interpreter resolves it by `Name` when the node runs.

`PinValue` provides coercion helpers used throughout the interpreter — `as_float()`, `as_int()`, `as_bool()`, `as_string()`, `as_vec2()`, `as_vec3()`, `as_color()` — so a node can read an input regardless of the exact upstream type.

## Implementing behaviour

The interpreter walks each graph from its event nodes. It evaluates data pins lazily (pulled, then cached for the tick) and follows execution wires forward. You hook your node into one of two dispatch functions, both keyed on the `node_type` string.

### Pure (data) nodes

Add an arm to `eval_node_output(&mut self, node_type, node_id, pin_name)`. Read inputs with `self.resolve_input(node_id, "pin")` (which follows a wire, else uses the inline value, else the pin default) and return a `PinValue`:

```rust
// in fn eval_node_output(&mut self, node_type: &str, node_id: NodeId, pin_name: &str) -> PinValue
"math/square" => {
    let v = self.resolve_input(node_id, "value").as_float();
    PinValue::Float(v * v)
}
```

If your node has several output pins, branch on `pin_name` to return the right one (see `math/split_vec3` or `transform/get_position` for the pattern).

### Action (exec) nodes

Add an arm to `execute_node(&mut self, node_id, exec_pin)`. Resolve inputs, emit an effect, then continue the flow by calling `self.follow_exec(node_id, "then")` (or whichever `exec_out` pin name you declared). The interpreter does not mutate the world directly — it queues one of three output kinds that the same downstream systems consume from Lua and Rhai:

- `self.transform_writes.push(TransformWrite { .. })` — position / rotation / scale / translate / look-at changes.
- `self.character_commands.push(CharacterCommand { .. })` — character-controller commands.
- `self.push_action("verb", [..])` (and the `push_action_mixed` / `push_action_vec3` / `push_action_targeted` variants) — a `ScriptAction` event, the same generic action bus that audio/UI/physics/animation systems observe.

A `ScriptAction`-based action node, mirroring the built-in `debug/log`:

```rust
// in fn execute_node(&mut self, node_id: NodeId, _exec_pin: &str)
"debug/announce" => {
    let message = self.resolve_input(node_id, "message").as_string();
    log::info!("[Blueprint] {}", message);
    self.push_action("log", [("message", message)]);
    self.follow_exec(node_id, "then");
}
```

A `TransformWrite`-based action node, mirroring `transform/set_position`:

```rust
"transform/set_position" => {
    let pos = self.resolve_input(node_id, "position").as_vec3();
    self.transform_writes.push(TransformWrite {
        entity: self.entity,
        new_position: Some(Vec3::new(pos[0], pos[1], pos[2])),
        new_rotation: None,
        translation: None,
        rotation_delta: None,
        new_scale: None,
        look_at: None,
    });
    self.follow_exec(node_id, "then");
}
```

Forgetting the `follow_exec` call leaves your node a dead end — execution stops there.

## Adding a new category

Categories are just strings. The existing ones are declared as `CAT_*` constants near the top of `nodes.rs`, each with a matching `CLR_*` RGB color used as the node header. To add a category:

1. Add a `pub const CAT_MY_THING: &str = "My Thing";` and a `const CLR_MY_THING: [u8; 3] = [..];`.
2. Use them in your `BlueprintNodeDef`s.
3. Add `CAT_MY_THING` to the list returned by `categories()` (in display order) so the palette shows the new section.

`nodes_in_category` filters `ALL_NODES` by the `category` field, so no other wiring is needed.

## Optional: bake-to-Lua support

The editor can export a graph to a Lua file (`apply_blueprint_to_lua` → `scripts/bp_<name>.lua`) via `renzora_blueprint::compiler::compile_to_lua`. This is an **editor-only convenience**, not the live execution path — the shipped runtime always interprets the `BlueprintGraph` directly.

If you want the bake to handle your node, add a matching arm in `crates/renzora_blueprint/src/compiler.rs` that emits the equivalent Lua. If you skip this, your node still runs correctly under the interpreter; it simply won't appear in a baked Lua export.

## Building and registering

`BlueprintPlugin` self-registers with `renzora::add!(BlueprintPlugin)` at runtime scope, so blueprints (and your new nodes) run in both the editor's play mode and exported games — there is nothing extra to enable. Because nodes live in a workspace crate that is statically linked into the engine, rebuild the engine after editing:

```bash
renzora run            # editor
renzora run runtime    # shipped game
```

## See also

- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — authoring graphs in the editor and how they relate to scripts
- [Blueprint Node API](/docs/r1-alpha5/api/blueprint-nodes) — the full built-in node and pin reference
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how the `ScriptAction` / `TransformWrite` buses are consumed
