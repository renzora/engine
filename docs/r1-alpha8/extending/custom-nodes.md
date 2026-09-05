# Custom Blueprint Nodes

Add your own node types to the visual blueprint system by extending the `renzora_blueprint` crate's node registry.

## How node types are defined

A node is **one self-contained unit**, registered in one place: `crates/renzora_blueprint/src/nodes/mod.rs`. It is a `NodeEntry` pairing

1. a **`BlueprintNodeDef`** — the pins, category, label and colour the editor draws, and
2. its **Lua emission** — a `data` function for a pure node's output expression, an `exec` function for a side-effecting node's statements.

Both halves live beside each other in the same file and go into one slice, `REGISTRY`. The editor palette (`node_def` / `categories` / `nodes_in_category`) and the compiler both read from it, so a node cannot be half-added: if it is in the registry it is in the palette *and* it compiles.

> **A blueprint compiles to Lua.** There is no live interpreter — no `execute_node`, no `eval_node_output`, no `follow_exec`, no `transform_writes` buffer. Those belonged to the interpreter that was removed when blueprints moved to a compile-to-Lua path, and any draft describing them is out of date. Your node's job is now to **emit Lua source**.

> There is also no `register_blueprint_node`, no execution trait, and no `NodeContext`. Adding a node means editing the `renzora_blueprint` workspace crate and rebuilding the engine — a fork-the-engine task, not a plugin hook. (`BlueprintNode` *is* a real type, but it is the serialized **node instance** in a graph, not a trait.)

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

Declare it as a `static` and build its pins with the `PinTemplate` helpers:

```rust
static SQUARE: BlueprintNodeDef = BlueprintNodeDef {
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

### Pin templates

| Builder | Makes |
|---------|-------|
| `PinTemplate::exec_in(name, label)` | An execution **input** (the white flow wire entering the node) |
| `PinTemplate::exec_out(name, label)` | An execution **output** |
| `PinTemplate::input(name, label, PinType)` | A data **input** |
| `PinTemplate::output(name, label, PinType)` | A data **output** |
| `.with_default(PinValue)` | The fallback for a data input with nothing wired |

A node with an `exec_in` is an **exec node** — it emits statements when flow reaches it. A node with only data pins is a **pure node** — it emits an expression wherever its output is read. An **event** node has an `exec_out` and no `exec_in`; it becomes a Lua hook.

### Pin types and values

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
| `Entity` | `PinValue::Entity` | `String` (resolved by **name**) |
| `Any` | `PinValue::None` (when empty) | wildcard — accepts any non-exec type |

> `Float` is `f32` and `Int` is `i32` — not 64-bit. An `Entity` pin carries a **name string**, not a live `Entity` handle.

## Emitting Lua

Both emission functions take a `&mut Compiler` and the node's id. The compiler is a small, four-method API:

| Call | Does |
|---|---|
| `c.data(node, pin)` | The Lua **expression** for a data input — follows the wire, else the inline value, else the pin default |
| `c.inline(node, pin)` | The literal on the pin, ignoring any wire. For names that must be compile-time constants |
| `c.emit(line)` | Append one statement at the current indentation |
| `c.exec(node, pin)` | Continue the flow out of an exec output pin, emitting whatever is wired there |
| `c.has_exec(node, pin)` | Whether anything is wired to that exec output — for skipping an empty branch |
| `c.indent_inc()` / `c.indent_dec()` | Indentation, for nodes that emit a block |

### A pure node

Return the expression as a `String`. Nothing is emitted; it is substituted where used.

```rust
fn square_data(c: &mut Compiler, n: NodeId, _pin: &str) -> String {
    let v = c.data(n, "value");
    format!("({v} * {v})")
}
```

Parenthesise. Your expression may land inside a larger one, and `a + b * c` does not mean what an unparenthesised emission would suggest.

If your node has several output pins, branch on `pin` to return the right expression for each.

### An exec node

Emit statements, then continue the flow. **Forgetting `c.exec` leaves your node a dead end** — everything wired after it silently never runs.

```rust
fn announce_exec(c: &mut Compiler, n: NodeId) {
    let msg = c.data(n, "message");
    c.emit(&format!("print_log(tostring({msg}))"));
    c.exec(n, "then");
}
```

A node that emits a block manages its own indentation, and can skip an arm nothing is wired to — this is `flow/branch`:

```rust
fn branch_exec(c: &mut Compiler, n: NodeId) {
    let cond = c.data(n, "condition");
    c.emit(&format!("if {cond} then"));
    c.indent_inc();
    c.exec(n, "true");
    c.indent_dec();
    if c.has_exec(n, "false") {
        c.emit("else");
        c.indent_inc();
        c.exec(n, "false");
        c.indent_dec();
    }
    c.emit("end");
}
```

### What you can emit

Anything in the [Scripting API](../api/scripting.md), because the generated source runs in the same VM as a hand-written script. That is the whole payoff of compiling rather than interpreting: a new engine function is reachable from a blueprint the moment it exists, with no second implementation to write.

Two conventions worth copying from the built-ins:

**Unwrap a Vec3 defensively.** An upstream node may hand you a `vec3()` table or a plain array, so the built-ins emit `({v}).x or {v}[1]` for each component rather than assuming one shape.

**Scale rates by `delta` in the node, not in the graph.** `transform/rotate` takes degrees *per second* and multiplies by `delta` itself, so `On Update → Rotate` is a complete frame-rate-independent spin with no Multiply node wired in. Pushing that arithmetic into the node is usually the difference between a two-node graph and a six-node one.

## Registering it

Add the `NodeEntry` to `REGISTRY`:

```rust
pub(crate) static REGISTRY: &[NodeEntry] = &[
    // …
    NodeEntry { def: &SQUARE, data: square_data, exec: None },
    NodeEntry { def: &ANNOUNCE, data: data_none, exec: Some(announce_exec) },
];
```

A pure node passes `exec: None`; an exec node passes `data: data_none`, the shared stub that emits `"nil"`.

That single edit makes the node discoverable everywhere: `node_def("math/square")` resolves it, `nodes_in_category("Math")` includes it, and the palette lists it.

## Write the test

**A node is not finished without a `#[cfg(test)]` test asserting the Lua it compiles to.** That is the convention the rebuilt registry is held to, and it is the only thing that catches a node whose emission drifts — a graph that compiles to subtly wrong Lua fails at runtime, far from the node that caused it.

```sh
cargo test --profile dist -p renzora_blueprint
```

or `renzora test -p renzora_blueprint` to run it the way CI does.

## Adding a category

Categories are just strings, paired with an RGB header colour near the top of `nodes/mod.rs`:

```rust
const CLR_MY_THING: [u8; 3] = [90, 140, 200];
```

Use `"My Thing"` as the `category` on your defs. `categories()` returns them in first-seen registry order and `nodes_in_category` filters on the field, so nothing else needs wiring — where your section appears in the palette is decided by where your entries sit in `REGISTRY`.

## Building

`BlueprintPlugin` self-registers with `renzora::add!(BlueprintPlugin)` at runtime scope, so blueprints and your new nodes run in both the editor's play mode and exported games. Nodes live in a statically linked workspace crate, so rebuild the engine after editing:

```sh
cargo renzora
```

## See also

- [Visual Blueprints](/docs/r1-alpha8/scripting/blueprints) — authoring graphs, and the compile-to-Lua model
- [Blueprint Node API](/docs/r1-alpha8/api/blueprint-nodes) — the 15 nodes that exist today
- [Scripting API](/docs/r1-alpha8/api/scripting) — everything your emission can call
