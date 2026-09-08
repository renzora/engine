# Blueprint Node API

Every node in the blueprint palette, what it compiles to, and the pins it exposes.

> **The palette was rebuilt.** Blueprints used to be walked by a live interpreter over 191 hand-written node definitions. That interpreter is gone: a graph now **compiles to Lua** and runs through the script VM, and the nodes were re-added from scratch — each one a single self-contained unit with a test asserting the Lua it emits. This page lists **all 15 nodes that currently exist**. Anything not here was not ported yet; write it in [Lua](/docs/r1-alpha8/scripting/lua) and attach both, or [add the node](/docs/r1-alpha8/extending/custom-nodes).

## How to read this page

Each node has **pins**: white `exec` pins carry control flow, coloured pins carry data.

- An **event** node has no `exec` input — it is a starting point, and maps to a script lifecycle hook.
- An **exec** node has an `exec` input and one or more exec outputs (usually `then`), and emits a statement.
- A **pure** node has no exec pins at all. It emits an *expression*, evaluated where it is used.

The "Compiles to" column is the actual Lua emitted, with `{pin}` standing for whatever is wired into that pin (or its default). Every function it calls is in the [Scripting API](scripting.md) — a blueprint cannot reach anything a script cannot.

## Event

Event nodes are graph entry points. Each becomes a Lua lifecycle hook.

| Node | `node_type` | Pins | Becomes |
|---|---|---|---|
| **On Ready** | `event/on_ready` | out: `exec` | `function on_ready()` |
| **On Update** | `event/on_update` | out: `exec`, `delta` (Float), `elapsed` (Float) | `function on_update()` |
| **On Event** | `event/on_event` | in: `name` (String); out: `exec`, `value` (Any) | `function on_event(name, args)`, filtered by `name` |
| **Emit Event** | `event/emit` | in: `exec`, `name` (String), `value` (Any); out: `then` | `emit({name}, { value = {value} })` |

**On Update** hands you the frame's `delta` and `elapsed` directly on output pins, so a time-dependent graph needs no extra plumbing.

**On Event** and **Emit Event** are a matched pair. Emit writes the payload as `{ value = … }` and On Event reads `args.value` back out, so the two line up without you having to know the table's shape. Events are broadcast — every script and blueprint that listens for that name hears it, one frame later.

## Math (pure)

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Add** | `math/add` | in: `a`, `b` (Float, default 0); out: `result` (Float) | `({a} + {b})` |
| **Multiply** | `math/multiply` | in: `a`, `b` (Float, default 1); out: `result` (Float) | `({a} * {b})` |
| **Combine Vec3** | `math/combine_vec3` | in: `x`, `y`, `z` (Float, default 0); out: `result` (Vec3) | `vec3({x}, {y}, {z})` |

## Transform (exec)

All three act on the entity the blueprint is attached to.

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Set Position** | `transform/set_position` | in: `exec`, `position` (Vec3); out: `then` | `set_position(x, y, z)` |
| **Set Rotation** | `transform/set_rotation` | in: `exec`, `rotation` (Vec3, euler degrees); out: `then` | `set_rotation(x, y, z)` |
| **Rotate** | `transform/rotate` | in: `exec`, `degrees` (Vec3, default `0, 90, 0`); out: `then` | `rotate(x * delta, y * delta, z * delta)` |

**Rotate takes a rate, not an angle.** Its `degrees` pin is degrees *per second*, and the compiler multiplies each axis by `delta` for you — so `On Update → Rotate` is a complete, frame-rate-independent spin with nothing else wired in. Setting `(0, 90, 0)` turns the entity a quarter-circle per second on any machine.

A Vec3 input is unwrapped as `({v}).x or {v}[1]`, so it accepts either a `vec3()` table or a plain array.

## Flow

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Branch** | `flow/branch` | in: `exec`, `condition` (Bool, default true); out: `true`, `false` | `if {condition} then … else … end` |

The `else` arm is emitted only when something is wired to the `false` pin, so an unused branch costs nothing in the generated source.

## Variable

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Get Variable** | `variable/get` | in: `name` (String); out: `value` (Any) | the Lua local named `{name}` |
| **Set Variable** | `variable/set` | in: `exec`, `name` (String), `value` (Any); out: `then` | `{name} = {value}` |

Variable names are sanitized into valid Lua identifiers, so a name with spaces or punctuation still compiles. A variable is an ordinary Lua local in the generated script — it does **not** persist across scenes, and it is not visible to other entities. To share state, emit an event or write a component field with `set`.

## Debug

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Log** | `debug/log` | in: `exec`, `message` (String, default `"Hello!"`); out: `then` | `print_log(tostring({message}))` |

Output lands in the editor Console. `tostring` is applied for you, so wiring a number or a Vec3 into `message` works.

## Animation

| Node | `node_type` | Pins | Compiles to |
|---|---|---|---|
| **Crossfade Animation** | `animation/crossfade` | in: `exec`, `name` (String), `duration` (Float, default 0.3), `looping` (Bool, default true); out: `then` | `crossfade_animation({name}, {duration}, {looping})` |

## What is not in the palette

Everything else. There is no node today for input, physics, audio, spawning, timers, collision, HTTP, networking, UI or math beyond add/multiply/combine — those were part of the old interpreter's palette and have not been re-added.

That is a smaller gap than it looks, because a blueprint and a script are the same thing once compiled:

- **Put the missing logic in a `.lua` file** and attach both a `BlueprintGraph` and a `ScriptComponent` to the entity. They run side by side against the same world.
- **Or add the node.** One `NodeEntry` plus a test is the whole job — see [Custom Blueprint Nodes](/docs/r1-alpha8/extending/custom-nodes).

## See also

- [Visual Blueprints](/docs/r1-alpha8/scripting/blueprints) — the editor and the execution model
- [Custom Blueprint Nodes](/docs/r1-alpha8/extending/custom-nodes) — adding to this list
- [Scripting API](/docs/r1-alpha8/api/scripting) — everything a node can compile down to
