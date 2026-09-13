# Building Plugins

Almost every feature in Renzora is its own Bevy plugin. This page covers the kind that is **compiled into the engine binary**, and what any plugin can do once it is loaded.

> **Writing a plugin to share, or to install?** That is a **[native plugin](native-plugins.md)** — shipped as source, compiled on the machine that installs it, and needing no engine checkout at all. Read that page instead. This one is for engine features, and assumes you have the repository.

## The plugin model

A Renzora plugin is a Bevy `Plugin`: anything implementing `bevy::app::Plugin`. You declare it once with `renzora::add!(...)` and the engine wires it in. There is no `app.add_plugins(...)` call to make by hand.

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MyState>()
            .add_systems(Update, my_system);
    }
}

#[derive(Resource, Default)]
pub struct MyState {
    pub counter: u32,
}

fn my_system(mut state: ResMut<MyState>) {
    state.counter += 1;
}

// Register with the engine. Runtime scope by default.
renzora::add!(MyPlugin);
```

The plugin type must implement `Default` — the macro constructs it with `Default::default()`.

> There is **no `renzora::prelude`**. Import the engine surface with `use renzora::*;`, or pull individual items (`use renzora::Inspectable;`). For ECS types (`Plugin`, `App`, `Query`, `Commands`, …) use Bevy's own `use bevy::prelude::*;`.

## The `add!` macro

`renzora::add!` is the one registration point. Its forms:

```rust
renzora::add!(MyPlugin);                               // Runtime scope (default)
renzora::add!(MyEditorTool, Editor);                   // Editor scope
renzora::add!(MyGameplay, Runtime);                    // Runtime scope, stated
renzora::add!(MyFoundation, Runtime, priority = -100); // with explicit order
```

**It is not a runtime registry.** The line is read *as text* at build time by a generator, which writes two committed files — `crates/renzora_runtime/src/plugins.rs` and `crates/renzora_editor/src/plugins.rs` — each an ordinary list of `app.add_plugins(...)` calls. So a named plugin in a generated list is just a linker symbol: there is no FFI, no registry to iterate, and nothing that can silently fail to register.

Two consequences worth knowing:

- **Keep the declaration on one line, at the top level of the file.** The parse requires the full `add!(..);` form at line start, so a commented-out or string-embedded one is ignored.
- **Keep every module on the plugin's path `pub`.** The type is resolved from the module the file defines. A wrong path is a compile error in the generated file, never a silently missing plugin.

Multiple `add!` lines in one crate are fine — `renzora_ember` has four.

Because both generated lists are committed, a plain `cargo build` needs no generator run. CI checks that regenerating produces no diff, which is what stops a stale list from shipping.

## Scopes

A plugin's scope is exclusively one of two, and matching is **exact equality**:

| Scope | Loads in the editor | Loads in the shipped game / server | Use for |
|-------|:---:|:---:|---------|
| `Runtime` (default) | yes | yes | gameplay, rendering, UI, audio, networking — anything that runs in the actual game |
| `Editor` | yes | no | panels, inspectors, gizmos, import tools — editor-only tooling |

`Runtime` plugins run in the editor viewport **and** the exported game. `Editor` plugins run only when the editor bundle is present beside the binary.

> There is no "both" scope. A feature that needs editor tooling *on top of* runtime behaviour ships **two** plugins, one of each — the convention in the engine is `renzora_<name>` plus a nested `renzora_<name>_editor` subcrate.

### Priority

`priority` is an `i32` order hint (default `0`, lower installs earlier). Reach for it only when a plugin must initialise a resource another plugin reads at install time. For ordinary system ordering, prefer Bevy's own `.before()` / `.after()` / `.chain()` and system sets.

## Adding the crate

A plugin crate is a plain `rlib` under `crates/`:

```toml
[package]
name = "renzora_myplugin"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
```

Dropping it in `crates/` with an `add!` line is the whole job — the generator finds it and adds the dependency. Then build:

```bash
cargo renzora dist
```

> An `Editor`-scope plugin becomes an **optional** dependency under the `editor` feature, so it is excluded from a lean runtime build.

### Scaffolding

```bash
renzora add <name>            # Runtime-scope crate under crates/
renzora add <name> --editor   # Editor scope, optional dep under [features].editor
```

## Keep dependencies thin

Minimise a plugin's dependency on other `renzora_*` crates. When a type must cross a crate boundary, **move it into the `renzora` contract crate** rather than depending on the crate that defines it — that is the established pattern, and it is what lets a native plugin reach the same type later without linking your crate.

## What a plugin can do

Inside `build(&self, app)` you have the full `&mut App` surface.

### Components and scene serialization

Derive the reflection traits and register the type so it survives scene save and load:

```rust
use bevy::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Component, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Health>();
    }
}
```

> Give any field you add later a `#[reflect(default)]`, or scenes saved before it existed will fail to load.

### Inspector UI

Custom inspectors use the `renzora` editor contract, gated behind the crate's `editor` feature:

```toml
renzora = { path = "../renzora", default-features = false, features = ["editor"] }
```

```rust
#[derive(Component, Reflect, Default, renzora::Inspectable)]
#[reflect(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}
```

### Viewport tools

`App::register_tool(ToolEntry)` adds a button to the viewport. A `ToolEntry` is an icon, a tooltip, and three closures: `visible_if` (show it at all), `active_if` (draw it highlighted) and `on_activate` (what clicking does). The **section** decides which surface it renders on:

| Section | Where it renders |
|---|---|
| `ToolSection::Transform` | the horizontal strip across the viewport's top edge, with Select / Move / Rotate / Scale |
| `ToolSection::Terrain` | the same strip, after a divider |
| `ToolSection::Custom(id)` | the same strip, after the built-in sections; `id` groups and sorts |
| `ToolSection::Shelf(group)` | the **two-column vertical shelf** down the viewport's left edge |

Everything else about an entry is identical either way, so moving a tool between surfaces is a one-word change.

> **The two surfaces split by depth, not by feature.** A tool that *opens* other tools stays on the strip — the gizmo modes, the terrain modes (Sculpt / Paint / Foliage), mesh Edit Mode — so there is always one visible row saying what the viewport is set to do. What each of those reveals goes on the shelf: the terrain sculpt brushes, the paint brushes, the foliage types, and in Edit mode the draw tools, select modes and ops. A tool that opens nothing and has no palette under it is better on the shelf with its neighbours than alone on the strip.
>
> **Keep shelf groups even.** The shelf is two buttons wide and every group starts on a fresh row, so an odd group ends on a row with a hole in it that reads as a missing button — and a group of one reads as a mistake. Where a group will not come out even, move a member to the neighbouring group where it also makes sense, or pair it with the control it belongs next to.

```rust
app.register_tool(
    ToolEntry::new("mytool.brush.smooth", "waves", "Smooth", ToolSection::Shelf("mytool.brushes"))
        .order(3)
        .visible_if(|w| /* only while my tool is active */ true)
        .active_if(|w| /* is this the chosen brush? */ false)
        .on_activate(|w| { /* choose it */ }),
);
```

Use the **strip** for the mode that turns your tool on, and the **shelf** for what that mode opens. The strip runs out of room past a few buttons and wraps into a second row, taking Play and the view controls down with it; the shelf grows downward where nothing competes for the space.

Shelf groups render top to bottom in **alphabetical order of the group string**, separated by a rule, and the whole shelf collapses when none of its entries are visible. That sort is *global*, across every crate that registers a group — so if your feature has several groups that must stay in a fixed order, encode it in the id. The terrain toolset does exactly this: `terrain.a-region` → `terrain.b-sculpt` → `terrain.c-paint` → `terrain.d-foliage-brush` → `terrain.e-foliage-types`, the last two registered by a different crate but part of the same palette and therefore carrying the same `terrain.` prefix.

### Viewport toolbar groups

A tool's *settings* — as opposed to the tool button itself — can be mounted as a group in the toolbar with `renzora_ember::toolbar::register_viewport_tool_group(key, builder)`. The `key` is a stable identifier: the group is draggable like every other group on the bar, and its position is saved under that key, so changing it resets users' toolbars.

```rust
renzora_ember::toolbar::register_viewport_tool_group("mytool-settings", |commands, fonts| {
    let group = commands.spawn(/* … */).id();
    // Hide the group when it isn't relevant — an always-visible group holds
    // its width in every other context for nothing.
    bind_display(commands, group, |w| /* my tool is active */ true);
    group
});
```

This exists because `renzora_viewport` cannot depend on the crates that want to mount things in it. Two narrower registries sit beside it: `register_viewport_tool_trailing` (widgets pinned to the strip's right-hand end) and `register_viewport_top_strip` (full-width bars under the strip).

## See also

- **[Native Plugins](native-plugins.md)** — the installable kind, shipped as source
- **[Editor Panels](panels.md)** — adding a dockable panel
- **[Script API Bindings](script-bindings.md)** — exposing functions to scripts
- **[Post-Processing Effects](post-processing.md)** — camera effects
- **[Custom Material Nodes](material-nodes.md)** — the material graph
