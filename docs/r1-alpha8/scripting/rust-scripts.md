# Rust Scripts

A `.rs` file in your project is a script. The editor compiles it when you save, and calls it once per frame for every entity it is attached to, with full access to the world.

## Create one

In the **Assets** panel, click **Import** and pick **Rust Script** from the list of new assets. You get `new_script.rs` in the current folder, and it opens in the code editor with a working starter in it.

<!-- screenshot: assets_new_menu.png - the Assets Add menu with Rust Script highlighted -->

```rust
use bevy::prelude::*;
use renzora::ScriptCtx;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotate_y(dt);
    }
}

renzora::script!(update);
```

Three parts matter:

- `update` runs once per frame, per entity. Name it anything.
- `ctx` is that entity, plus the world.
- `renzora::script!(update)` exports it. Without this line the script compiles and then loads as "exports no entry point".

Press `Ctrl+S`. The script compiles in the background. The **Console** reports the result.

Scripts can live anywhere in your project, not just in a `scripts/` folder. Put one beside the model it drives if that suits you better.

## Attach one

Either way works:

- Drag the `.rs` file from the **Assets** panel onto an entity in the Hierarchy or the viewport.
- Select the entity, find the **Scripts** component in the Inspector, and click **Add Script**.

<!-- screenshot: script_component.png - the Scripts component in the Inspector with a .rs script attached, showing its play toggle -->

One entity can carry several scripts, and they can be in different languages. Routing is by file extension.

## When it runs

A script does nothing while you are arranging the scene. It runs in the game you start with **Play**, or in the editor when you switch on that one script's play button in the Inspector.

That last one is how you test a single script without running the whole game.

**Play** runs the game as a separate process. It loads the libraries the editor already built for your scripts, so nothing compiles a second time; save a script before pressing Play and the game gets the new build.

## Recompiling

Saving rebuilds the script. The compile runs off the main thread, so the editor does not freeze.

Opening a project builds its scripts the same way, in the background. A script that has not changed since the last launch is not compiled again: its previous build is reused, as long as the engine has not been updated since. Scripts that do need a build finish a moment after the project opens, so pressing Play in the first second or two can start before they are ready.

Compile errors, panics and a missing SDK all appear in the **Console**, with the line numbers pointing at your file.

A script that fails to compile is not retried until you edit it again, so one error does not turn into a scrolling wall.

A script that **panics** while running is not a crash. The editor catches it, and the Console names the script, the panic message and the line in your file:

```
panics_on_purpose.rs panicked on 2984v0: index out of bounds: the len is 0 but the index is 3 (at panics_on_purpose.rs:8:18)
```

The panic stops that call, not the editor, and the rest of the scene keeps running. A script that panics every frame reports every frame, and the Console folds the repeats into a count. Fix the file and save, and it picks up the new build.

## What you can reach

`ctx` acts on your own entity with no argument.

| | |
|---|---|
| `ctx.get::<T>()` / `get_mut::<T>()` | A component on this entity |
| `ctx.has::<T>()` | Does this entity have it |
| `ctx.insert(bundle)` / `remove::<T>()` | Add or remove components on this entity |
| `ctx.name()` | This entity's name, if it has one |
| `ctx.entity()` | This entity's id |
| `ctx.children()` / `ctx.parent()` | The hierarchy around it |
| `ctx.delta()` / `ctx.elapsed()` | Seconds since last frame, seconds since startup |
| `ctx.get_on::<T>(e)` / `get_mut_on::<T>(e)` | A component on some other entity |
| `ctx.get_resource::<T>()` / `get_resource_mut::<T>()` | A resource, if it exists |
| `ctx.world()` | The whole `&mut World` |

`world()` is not a last resort. Spawning, querying and asset access go through it.

```rust
fn update(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    let world = ctx.world();
    world.spawn((Name::new("spawned by a script"), ChildOf(me)));
}
```

`insert` and `remove` do nothing if the entity has already been despawned, so you do not have to check you still exist before every write.

There is no vocabulary in the way. Anything Bevy allows, a script can do.

## Lifecycle hooks

`update` covers every frame. For everything that is not every frame, export a second function.

```rust
use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

fn update(ctx: &mut ScriptCtx) {
    let _ = ctx;
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            info!("{:?} is ready", ctx.entity());
        }
        ScriptHook::PlayerJoined { id } => {
            info!("player {id} joined");
        }
        _ => {}
    }
}

renzora::script!(update, hooks = hooks);
```

One function takes them all, so adding a hook later is not a new export.

| Hook | Fires when |
|---|---|
| `Ready` | The first frame this script runs on this entity, before its first `update`. |
| `FixedUpdate` | Once per fixed step, after `Ready`. Carries `delta`, the fixed step in seconds. Commands it queues reach the physics step that follows. |
| `Ui` | A markup callback fired, such as `on_press`. Carries the callback name, its arguments and the node that fired it. |
| `Draw` | Time to repaint this entity's canvas surface. Carries the surface size in pixels. |
| `SceneLoaded` | A scene finished loading, or failed. Carries the path and the error. |
| `AnimationEvent` | Animation playback crossed a clip marker. |
| `Http` | A background HTTP request completed. Carries the status and body. |
| `Rpc` | A networked call arrived. Carries the name, arguments and the sender's peer id. See [Multiplayer](/docs/r1-alpha8/multiplayer/overview). |
| `PlayerJoined` / `PlayerLeft` | A peer connected or disconnected. On the server (dedicated or host) only. |
| `Event` | A game event: one a script emitted, or an answer from a plugin. Carries the name, arguments and the entity that sent it. The same events Lua's `on_event` gets. |

`SceneLoaded` only reaches scripts the load did not destroy, which in practice means scripts in a global scene. That is what makes a loading screen possible.

## Calling a plugin's functions

A plugin adds script functions by declaring them (`mysql_query`, `http_*`, a weather plugin's `set_rain`). Each one is a `ScriptAction` underneath, so a Rust script calls it by sending that action with the same arguments:

```rust
use renzora::script_bridge::ScriptAction;

ScriptAction::new("mysql_query", ctx.entity())
    .arg("sql", "SELECT name, score FROM players ORDER BY score DESC LIMIT 10")
    .arg("callback", "top_ten")
    .send(ctx.world());
```

A function that does slow work cannot return its result. The plugin answers later with an event named by the callback:

```rust
fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Event { name: "top_ten", args, .. } = hook {
        if let Some(ScriptActionValue::String(rows)) = args.get("rows") {
            let rows: Vec<renzora::serde_json::Value> =
                renzora::serde_json::from_str(rows).unwrap_or_default();
            info!("{} players", rows.len());
        }
    }
}
```

A plugin's documentation lists its functions, their arguments and what its answer carries.

### HTTP requests

`http_get` and `http_post` work the same way. The request runs in the background, and its answer arrives as the `Http` hook, tagged with the callback name you chose:

```rust
ScriptAction::new("http_get", ctx.entity())
    .arg("url", "https://example.com/api/leaderboard")
    .arg("callback", "leaderboard")
    .send(ctx.world());

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Http { callback: "leaderboard", status, body } = hook {
        // A status of 0 means the request never completed; `body` is the error.
    }
}
```

`http_post` takes a `body` argument as well.

### The plugin bus

Events a plugin emits on [the plugin bus](/docs/r1-alpha8/extending/plugin-bus) arrive as `ScriptHook::Event` too, and an event a script emits reaches plugins. A Rust script can also call a plugin's bus service directly and get the answer straight back, since it holds `&mut World`:

```rust
use renzora::bus::prelude::*;

match ctx.world().call("inventory.add_item", BusValue::map().with("id", "sword")) {
    Ok(answer) => info!("added: {answer:?}"),
    Err(err) => warn!("{err}"),
}
```

## Drawing

`ctx.painter()` gives you an immediate-mode 2D painter for entities that have a canvas surface, with `line`, `arc`, `circle`, `rect`, `rect_outline`, `triangle`, `poly` and `text`.

The list is rebuilt from nothing every frame, so draw your whole picture each time and never remove anything. Use the `Draw` hook, which hands you the surface's real size.

## Requirements

A Rust script is compiled on the machine that opens the project, so that machine needs a compiler.

- The **plugin SDK** must be installed, under **Settings > Plugins**. Without it nothing compiles, and the Console says so once.
- The pinned Rust version must be present. The editor checks when it starts, and if Rust is missing, a strip across the top of the start screen says so and offers **Install Rust**.
  - **Install Rust** adds the version through rustup if you have rustup, or downloads the official installer from rust-lang.org and sets Rust up in `~/.cargo` and `~/.rustup` if you do not (about 400 MB). It takes a few minutes, and you can keep using the start screen meanwhile. The strip turns green when it is done. Scripts then compile without a restart, and a marketplace plugin that was waiting for Rust builds the next time the editor starts.
  - If you open a project without installing, and the project has scripts, the editor asks once more with an **Install Rust?** prompt, and the scripts wait while you decide. **Not now**, or closing the prompt, lets them build anyway: they fail with an error naming the command that installs the version.
  - A plugin installed from the marketplace while Rust is missing still installs. It is not marked as failed; it builds on the first start after Rust is there.
- The editor must have been built for the platform you are running it on. An editor cross-built for another operating system carries an SDK that cannot compile scripts here. The symptom is every name in `bevy::prelude` reported missing at once.

Build output lands in `<project>/.renzora/scripts/`. Nothing in there needs looking at or committing.

## In an exported game

Scripts run in exports, and the player needs no compiler and no SDK.

| Export mode | How the script gets in |
|---|---|
| Separate files, or single binary | Shipped as a library beside the game, compiled by the editor at export time |
| Lean single binary | Compiled into the executable by the export build |

Every `.rs` in the project is included, not only the ones a scene currently references, because a scene can be loaded and a script attached at runtime. An unused script costs bytes, never frame time.

One limit: a **cross-platform copy-based export ships no scripts**, because the library it would ship is the wrong shape for the target machine. Export **lean** for another platform instead, which compiles scripts into the binary and has no such limit.

## Limits

- **No script properties.** A script's tunables are ordinary components on the entity, which the Inspector already edits.
- **One file per script.** If you need modules, write a [plugin](/docs/r1-alpha8/extending/plugins) instead.
- **No unloading.** Each reload leaves its old code in memory, about 200 KB. Restarting the editor reclaims it.

## Other languages

Rust is what the engine compiles on its own. Other languages arrive as backends from the [Marketplace](/docs/r1-alpha8/marketplace/browsing): install one and the engine routes files with that extension to it.

Writing one is covered in [Script Backends](/docs/r1-alpha8/extending/script-backends).
