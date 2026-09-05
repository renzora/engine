# Building Plugins

Almost every feature in Renzora is its own Bevy plugin; this page shows how to write one and register it with a single macro.

## The plugin model

A Renzora plugin is just a Bevy `Plugin` (anything that implements `bevy::app::Plugin`). You declare it once with `renzora::add!(...)` and the engine wires it in automatically — there is no central list of plugins to edit and no `app.add_plugins(...)` call to make by hand.

> **There are now three kinds of plugin.** This page covers the two that existed first — the workspace plugin compiled into the binary, and the standalone C-ABI plugin. The third, a **[native plugin](native-plugins.md)**, is an ordinary Bevy plugin shipped as *source* and compiled on the machine that installs it: it gets full `&mut World`, can add [editor panels](panels.md), and needs no engine source edits. That is the one to reach for when extending the **editor**. A C-ABI plugin is still the only kind that ships inside a **game**.

There are exactly two kinds of plugin, and the difference is purely how the crate is compiled and linked:

| Kind | Crate type | Linked | Registers via | Ships in |
|------|-----------|--------|---------------|----------|
| **Workspace plugin** | `rlib` (default) | statically, into the `renzora` binary | inventory constructor at process start | the binary itself |
| **Distribution plugin** | `cdylib` (own `dlopen` feature) | dynamically, `dlopen`'d at runtime | `extern "C"` FFI exports | a `.dll`/`.so`/`.dylib` dropped into `<exe>/plugins/` |

> Recap of the engine shape: there is **one** binary (`renzora`) and it is always runtime-shaped. The editor is itself a removable `renzora_editor` cdylib bundle that sits beside the executable. Your plugins slot into the same model — either compiled into the binary, or loaded beside it.

## A minimal plugin

Both plugin kinds share identical Rust. The plugin type must implement `Default` (the macro constructs it with `Default::default()`):

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

> There is **no `renzora::prelude`**. Import the SDK surface with `use renzora::*;`, or pull individual items (`use renzora::Inspectable;`). For ECS types (`Plugin`, `App`, `Query`, `Commands`, …) use Bevy's own `use bevy::prelude::*;`.

## The `add!` macro

`renzora::add!` (defined in `crates/renzora/src/plugin_meta.rs`) is the one registration point. Its forms:

```rust
renzora::add!(MyPlugin);                          // Runtime scope (default)
renzora::add!(MyEditorTool, Editor);              // Editor scope
renzora::add!(MyGameplay, Runtime);               // Runtime scope, stated
renzora::add!(MyFoundation, Runtime, priority = -100); // with explicit order
```

The macro expands to **two parallel registration paths**, and the build target decides which one is live:

1. **Inventory (always, every platform).** It emits an `inventory::submit!{ StaticPlugin { name, scope, priority, install } }`. At startup the host iterates the global registry with `for_each_static_plugin(host_scope, …)` and installs every matching plugin in priority order. This is the path for statically-linked workspace plugins on desktop, iOS, Android, and wasm.

2. **FFI exports (desktop only, `dlopen` feature only).** Under `#[cfg(all(feature = "dlopen", not(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))))]` it emits three `#[no_mangle] extern "C"` functions the dynamic loader reads:

   ```rust
   #[no_mangle] pub extern "C" fn plugin_create() -> *mut dyn Plugin;
   #[no_mangle] pub extern "C" fn plugin_scope()  -> u8;       // PluginScope discriminant
   #[no_mangle] pub extern "C" fn plugin_bevy_hash() -> [u64; 2]; // ABI guard
   ```

   The `cfg(feature = "dlopen")` gate resolves against the **calling crate**, not against `renzora`. That is why a distribution plugin declares its own `dlopen` feature rather than enabling one on the `renzora` dependency.

Because those FFI symbols are unmangled, a distribution cdylib may contain **exactly one** `add!` — two would collide on `plugin_create`. Workspace plugins never enable `dlopen`, so the symbols stay off and multiple `add!`s link cleanly into the host.

## Scopes

`PluginScope` has exactly two variants and matching is **exact equality** — there is no "both" scope:

```rust
pub enum PluginScope {
    Editor  = 0,
    Runtime = 1,
}
```

| Scope | Loads in the editor session | Loads in the shipped game / server | Use for |
|-------|:---:|:---:|---------|
| `Runtime` (default) | yes | yes | gameplay, rendering, UI, audio, networking — anything that runs in the actual game |
| `Editor` | yes | no | panels, inspectors, gizmos, import tools — editor-only tooling |

`Runtime` plugins load in the runtime pass, which the editor host runs too, so they appear in the editor viewport **and** the exported game. `Editor` plugins load only when the editor bundle is present.

> There is no `EditorAndRuntime` scope. A feature that needs editor tooling *on top of* runtime behaviour ships **two** plugins — one of each scope (the convention in the engine is e.g. `GameUiPlugin` + `GameUiEditorPlugin`).

### Priority

`priority` is an `i32` order hint (default `0`, lower installs earlier). Reach for it only when a plugin must initialise a resource another plugin reads at install time. For ordinary system ordering, prefer Bevy's own `.before()`/`.after()`/`.chain()` and system sets instead.

## Workspace plugins (static)

A workspace plugin is the default: a plain `rlib` crate under `crates/`, statically linked into the binary. Minimal `Cargo.toml`:

```toml
[package]
name = "renzora_myplugin"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
```

To get linked into the binary, add the crate as a dependency of `renzora_runtime` (`renzora_myplugin = { path = "../renzora_myplugin" }`). Its `inventory` constructor then self-registers at process start — you do **not** call `add_plugins` anywhere. Build via the renzora CLI (which runs the build inside the pinned Docker toolchain):

```bash
renzora build    # binary + editor bundle (--workspace)
renzora build    # lean game binary only (--bin renzora)
```

> An `Editor`-scope workspace plugin is added to `renzora_runtime` as an **optional** dependency under its `editor` feature, so it is excluded from the lean runtime build.

## Distribution plugins (dynamic)

A distribution plugin ships as a standalone `cdylib` that the engine `dlopen`s from `<exe>/plugins/` — no rebuild of the engine required. The crate declares its **own** `dlopen` feature (default-on) so `add!` emits the FFI exports:

```toml
[package]
name = "renzora_myplugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[features]
default = ["dlopen"]
dlopen = []

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
```

The Rust is identical to a workspace plugin (one `Default` plugin type, one `renzora::add!`). Build it via the renzora CLI (the build runs inside the Docker toolchain) and copy the artifact into the engine's `plugins/` directory:

```bash
renzora build
# -> target/dist/renzora_myplugin.{dll,so,dylib}
# copy that file into  <renzora-binary>/plugins/
```

At startup `dynamic_plugin_loader` scans `<exe>/plugins/`, verifies the ABI guard, reads `plugin_scope` to decide whether to load (Editor → only in an editor session; Runtime → always), then calls `plugin_create` and runs the plugin's `build(app)`. The loaded `Library` is kept alive in a `DynamicPluginRegistry` for the life of the process.

`renzora_hot_demo` (`crates/renzora_hot_demo`) is a complete, working example of a distribution plugin that spawns and animates entities to prove a `dlopen`'d cdylib gets the same full `&mut App` / ECS access as a built-in plugin.

## Scaffolding with `renzora add`

`renzora add` generates a plugin skeleton (`crates/renzora_<name>/` with `Cargo.toml` + `src/lib.rs`):

```bash
renzora add <name>            # static workspace plugin
renzora add <name> --editor   # Editor-scope, optional dep under [features].editor
renzora add <name> --dylib    # distribution cdylib (default = ["dlopen"])
```

| Flag | Crate type | Scope | Wiring it does |
|------|-----------|-------|----------------|
| *(none)* | `rlib` | Runtime | adds a non-optional dep to `renzora_runtime` |
| `--editor` | `rlib` | Editor | adds an optional dep under `renzora_runtime`'s `editor` feature |
| `--dylib` | `cdylib` (`dlopen`) | Runtime | none — auto-included by the `crates/*` glob, loaded at runtime |

`--editor` and `--dylib` are mutually exclusive.

> ⚠️ The script's **no-flag** default currently writes `renzora::add!(<Name>Plugin, EditorAndRuntime);` into the skeleton. `EditorAndRuntime` is **not** a real `PluginScope` variant, so the generated crate will not compile as-is — change that line to `renzora::add!(<Name>Plugin);` (Runtime) or `renzora::add!(<Name>Plugin, Editor);` after scaffolding. The `--editor` and `--dylib` paths emit valid scopes.

## The ABI guard

A `dlopen`'d plugin and the host share the **same compiled `bevy_dylib` and `renzora` dylib** (via `prefer-dynamic` + `bevy/dynamic_linking`), so their `TypeId`s line up across the boundary. `plugin_bevy_hash()` enforces this at load time:

```rust
#[no_mangle]
pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
    let id = std::any::TypeId::of::<bevy::ecs::world::World>();
    unsafe { std::mem::transmute(id) }
}
```

The loader compares this against its own value and **rejects any plugin whose hash does not match** — a mismatch means the plugin was built against a different Bevy/engine and would corrupt ECS access. In practice this means: build distribution plugins with the **same engine version and toolchain** as the host (the `docker/base/Dockerfile` image, pinned to one Rust version, is the canonical build environment). The build also stamps a `RENZORA_BUILD_HASH` (version + rustc + Bevy) used for the same compatibility checks.

## Hot-loading

`HotPluginPlugin` watches `<exe>/plugins/` (~1s interval, on the `Last` schedule) and builds newly dropped dlls into the **live** `World`, so a main-world plugin activates on the next frame without a restart. Plugins that touch the render world (post-process effects, custom render-graph nodes) can't be spliced into an already-initialised renderer; they load as far as the main world allows and report `NeedsReload` so the editor can prompt for a restart.

## The editor bundle

The editor ships as a single bundle cdylib (`renzora_editor`) that exports `plugin_install_scope` instead of the `plugin_create` trio. It is produced by `renzora::export_plugin_bundle!(foundation = [...])`, which installs an ordered foundation and then replays every `Editor`-scope plugin from the one global inventory:

```rust
renzora::export_plugin_bundle!(foundation = [
    renzora_asset_registry::AssetRegistryPlugin,
    renzora_editor_framework::RenzoraEditorPlugin,
    renzora_keybindings::KeybindingsPlugin,
]);
```

You rarely write this yourself — it is how the editor itself is assembled. The dynamic loader deliberately **skips** any cdylib exporting `plugin_install_scope` when scanning `plugins/`, so a bundle only ever loads beside the exe and the editor is never accidentally shipped inside a game. Normal community plugins use `add!`.

## What a plugin can do

Inside `build(&self, app)` you have the full `&mut App` surface — exactly what a built-in plugin has. Common additions:

### Components and scene serialization

Derive the reflection traits and register the type so it survives scene save/load (Renzora serializes scenes to RON):

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

### Inspector UI

Custom inspectors are **not** egui (egui has been removed from the engine entirely — there is no `EditorPanel` trait and no `register_panel`). They use the `renzora` editor contract, which is gated behind the crate's `editor` feature (default features are empty):

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

> **The two surfaces split by depth, not by feature.** A tool that *opens* other tools stays on the strip — the gizmo modes, the terrain modes (Sculpt / Paint / Foliage), mesh Edit Mode — so there is always one visible row saying what the viewport is set to do. What each of those reveals goes on the shelf: the 17 terrain sculpt brushes, the paint brushes, the foliage types, and in Edit mode the two draw tools, the select modes and the ops. A tool that opens nothing and has no palette under it is better on the shelf with its neighbours than alone on the strip — that's why Generate Terrain and Resize Terrain sit with the terrain size controls rather than in the terrain mode row.
>
> **Keep shelf groups even.** The shelf is two buttons wide and every group starts on a fresh row, so an odd group ends on a row with a hole in it that reads as a missing button — and a group of one reads as a mistake. Where a group won't come out even, move a member to the neighbouring group where it also makes sense, or pair it with the control it belongs next to.

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

Shelf groups render top to bottom in **alphabetical order of the group string**, separated by a rule, and the whole shelf collapses when none of its entries are visible. That sort is *global*, across every crate that registers a group — so if your feature has several groups that must stay in a fixed order, encode it in the id. The terrain toolset does exactly this: `terrain.a-region` → `terrain.b-sculpt` → `terrain.c-paint` → `terrain.d-foliage-brush` → `terrain.e-foliage-types`, the last two registered by a different crate (`renzora_foliage_editor`) but part of the same palette, and therefore carrying the same `terrain.` prefix. The modeling groups do the same across two crates: `modeling.a-draw` comes from `renzora_mesh_draw`, `modeling.b-select` onward from `renzora_mesh_edit`.

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

This exists because `renzora_viewport` can't depend on the crates that want to mount things in it. Two narrower registries sit beside it in the same module: `register_viewport_tool_trailing` (widgets pinned to the strip's right-hand end) and `register_viewport_top_strip` (full-width bars under the strip).

See **Script API Bindings** for exposing functions to scripts, **[Post-Processing Effects](./post-processing.md)** for camera effects (which are [standalone plugins](./standalone-plugins.md) now, not distribution plugins), and **Custom Blueprint Nodes** / **Custom Material Nodes** for those subsystems — each has its own registration path layered on the same `add!` model described here.
