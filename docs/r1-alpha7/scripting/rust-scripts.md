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
- The editor must have been **built on the platform you are running it on**. An editor cross-built for another operating system carries an SDK whose proc macros are for the machine that compiled it, and no Rust script will compile against it. The tell is `can't find crate for bevy_derive`, followed by every name in `bevy::prelude` reported missing at once — a script that looks broken but is not. See [The SDK cannot be cross-built](../extending/native-plugins.md#the-sdk-cannot-be-cross-built).

Build artifacts land in `<project>/.renzora/scripts/`. They are derived — nothing there needs to be looked at or committed.

## In an exported game

Scripts run in exports. How they get there depends on the packaging mode, and neither route asks anything of the player — no SDK, no Rust toolchain, nothing to install.

| Packaging | How the script gets in | Compiled by |
|---|---|---|
| Separate files / Single binary | shipped as a library beside the game | the editor, at export |
| Lean single binary | compiled into the executable | the export build |

**Copy-based** exports carry the same `bevy_dylib` and `renzora_dylib` the editor compiled your script against, so it loads exactly as it does in the editor. The export copies the library the editor already built — a script that has never compiled has nothing to ship, and the export says so rather than omitting it quietly.

**Lean** exports link Bevy statically and share no image, so there is no library for a script to bind to. Instead each `scripts/*.rs` becomes a module of the binary and its entry point goes into a table the dispatcher reads. Everything after that is identical: one function per entity per frame, keyed by file name, inside the same panic guard. A script behaves the same in the editor and in an export, or an export could not be tested by playing it.

Every `.rs` in the project is compiled in, not only the ones a scene currently references — a scene can be loaded at runtime and a script attached at runtime, so any "which are used" analysis would eventually be wrong in the direction that breaks a game silently. An unused script costs bytes, never frame time: the dispatcher only ever looks up names a live entity asked for.

Scripts may live anywhere in the project, not only in `scripts/`. Where two folders hold the same file name, each is reachable by its full project-relative path but not by the bare name, which is ambiguous — the export names any that apply.

## Limits

- **A cross-platform copy-based export ships no scripts.** The libraries it would ship are host-shaped — a `.dll` is no use to a Linux player — and compiling for another OS needs an SDK for that target. Export *lean* for another platform instead, which compiles the scripts into the binary and has no such limit.
- **No props.** A Lua script declares tunables in a table the backend parses. The Rust equivalent would read attributes off the source; until then, a script's tunables are ordinary components on the entity, which the inspector already edits.
- **No REPL.** Evaluating a Rust expression would mean invoking the compiler and mapping a library per expression.
- **One file.** A script is a single `.rs`; a plugin is the answer when you need modules.
