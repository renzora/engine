# Rust Scripts

A `.rs` file in your project's `scripts/` directory is compiled on the machine that opens the project and called once per frame for each entity carrying it — with the same `&mut World` an exclusive system gets.

```rust
// <project>/scripts/spin.rs
use bevy::prelude::*;
use renzora::ScriptCtx;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if let Some(mut t) = ctx.get_mut::<Transform>() {
        t.rotate_y(dt);
    }
}

renzora::script!(update);
```

Attach it exactly like a Lua script: drop it into the entity's **Scripts** component. Routing is by file extension, so `.rs`, `.lua` and `.blueprint` scripts coexist on the same entity.

## What you get

Everything Bevy allows. Spawn hierarchies, build UI, insert and remove components, mutate assets, reach other entities, read and write resources. There is no vocabulary in the way, because the script and the engine share one Bevy.

That is the trade against Lua. A Lua script is sandboxed, hot-reloads instantly, and cannot take the editor down. A Rust script is native code with full access, costs about a second to compile, and a segfault in it is a segfault in the editor. Reach for one when the sandbox is what is stopping you.

## `ScriptCtx`

The context is your own entity plus the world. The short methods act on **yourself**, with no argument:

| | |
|---|---|
| `ctx.get::<T>()` / `get_mut::<T>()` | a component on this entity |
| `ctx.has::<T>()` | does this entity have it |
| `ctx.insert(bundle)` / `remove::<T>()` | add or remove components on this entity |
| `ctx.name()` | this entity's `Name`, if any |
| `ctx.entity()` | this entity's id, for handing to something else |
| `ctx.children()` / `ctx.parent()` | the hierarchy around it |
| `ctx.delta()` / `ctx.elapsed()` | seconds since last frame / since startup |
| `ctx.get_on::<T>(e)` / `get_mut_on::<T>(e)` | a component on some *other* entity |
| `ctx.get_resource::<T>()` / `get_resource_mut::<T>()` | a resource, if it exists |
| **`ctx.world()`** | the whole `&mut World` |

`world()` is not a last resort. Spawning, querying and asset access all go through it:

```rust
fn update(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    let world = ctx.world();
    let count = world.query::<&Transform>().iter(world).count();
    world.spawn((Name::new("spawned by a script"), ChildOf(me)));
}
```

`insert` and `remove` silently do nothing if the entity has been despawned — by an earlier script this frame, or by this one. You do not have to check you still exist before every write.

## When it runs

Exactly when a Lua script does: in play mode, in Simulate, or when that script's **play button** in the inspector is on. Nothing runs while you are arranging the scene in edit mode — a script that spawns or despawns would otherwise start doing so the moment you dropped it on an entity.

## Recompiling

Saving a script rebuilds it. The compile runs off the main thread, so the editor does not freeze; only the load and pointer swap happen on the main thread. Compile errors, panics and a missing SDK all appear in the **Console** panel as well as the log — with diagnostics pointing at `scripts/foo.rs`, not at the staged copy the compiler actually saw.

A script that fails to compile is not retried until you edit it again, so one error does not become a scrolling wall.

Every reload leaks its old image, roughly 200 KB. It has to: components the script inserted carry `Drop` impls and vtables living in that image, so unmapping it would turn a later despawn into a jump through freed memory. A restart reclaims all of it.

## Requirements

A Rust script is a [native plugin](../extending/native-plugins.md) with a per-entity convention on top — same compiler driver, same SDK, same loading. So the requirements are the plugin requirements:

- The **plugin SDK** must be installed (**Settings → Plugins**). Without it, nothing compiles and the Console says so once.
- The pinned `rustc` must be present. The editor names the version and offers to install it.

Build artifacts land in `<project>/.renzora/scripts/`. They are derived — nothing there needs to be looked at or committed.

## Limits

- **Nothing runs in a lean export.** A static single binary links no shared images, so a script library has nothing to bind to. Compiling scripts *into* the export is the answer, and the exporter is shaped for it, but it is not done.
- **No props.** A Lua script declares tunables in a table the backend parses. The Rust equivalent would read attributes off the source; until then, a script's tunables are ordinary components on the entity, which the inspector already edits.
- **No REPL.** Evaluating a Rust expression would mean invoking the compiler and mapping a library per expression.
- **One file.** A script is a single `.rs`; a plugin is the answer when you need modules.
