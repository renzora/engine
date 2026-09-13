# Architecture

How Renzora is put together: one Bevy 0.19 binary, a removable editor bundle, the `renzora.dll` contract, the `add!` plugin model, and the render/scene/asset pipelines.

Renzora is a large Cargo workspace where almost every feature is its own crate that registers a Bevy `Plugin`. The defining structural decision is the **"one binary, editor-as-removable-cdylib" model** (internally "Operation Merge", now fully shipped): the editor, the runtime, and the dedicated server are the *same* executable, and what it does is decided at startup, not at compile time.

## One binary

There is exactly **one** workspace binary, `renzora` (crate `renzora_app`, `[[bin]] name = "renzora"`, `src/main.rs`, `default-run = "renzora"`). It is the engine — editor, game runtime, and server in one — not a project launcher.

The binary is **always runtime-shaped**. The root `Cargo.toml` declares `default = ["runtime"]`, and the only build features are `runtime` and `wasm`. **There is no `editor` compile-time feature and no separate editor binary.**

### The editor is a removable file

The editor ships as the **`renzora_editor` cdylib bundle** (`crate-type = ["cdylib"]`) that sits *beside* the executable. At startup `editor_bundle_path()` looks for it:

| Platform | File looked for |
|----------|-----------------|
| Windows | `renzora_editor.dll` |
| Linux | `librenzora_editor.so` or `renzora_editor.so` |
| macOS | `librenzora_editor.dylib` or `renzora_editor.dylib` |

`editor_session()` returns true only if that file is present **and** neither `--no-editor` nor `RENZORA_NO_EDITOR` is set **and** it is not a `--server`/`--host` launch.

> **Present beside the exe → the binary is the editor. Delete that one file → the same binary is the shipped game.** The bytes of the executable are identical either way.

### Runtime modes

The mode is chosen at runtime in `src/main.rs`, never by a cargo feature:

| Launch | Behaviour |
|--------|-----------|
| (default windowed) | Editor if the bundle file is present, otherwise the shipped game |
| `--no-editor` / `RENZORA_NO_EDITOR` | Force game mode even if the bundle is present |
| `--server` | Headless dedicated server — `add_headless_rendering` (wgpu `backends: None`, no window, `WinitPlugin` disabled, a `ScheduleRunnerPlugin` at the network tick), plus `NetworkServerPlugin`. No GPU. |
| `--host` | Windowed listen server — full rendering plus the client half *and* `NetworkServerPlugin` in one process |

`--host` wins if both `--host` and `--server` are passed, and a server/host launch is **never** an editor session even when the bundle file is present. The dedicated server is the same `renzora` binary, not a separate executable. Server launches accept `--port`, `--addr`/`--address`, `--tick-rate`, and `--max-clients`, which overlay the `[network]` table of `project.toml`.

## Core crate layers

A handful of crates form the spine that everything else plugs into:

| Crate | Artifact | Role |
|-------|----------|------|
| `renzora` | `rlib` (shared through `renzora_dylib`) | The contract crate — the `add!` / `plugin!` macros, `PluginScope`, the GI contract types, the post-process framework, the audio/net/script boundary types, the `runtime_warnings` ring buffer, and (under the `editor` feature) the editor contract registries |
| `renzora_runtime` | `rlib` | Shared engine library every binary links: `init_app`, `add_default_rendering`, `add_headless_rendering`, `add_engine_plugins` |
| `renzora_engine` | `rlib` | The editor-free game core: VFS, custom asset reader, scene IO, autoload, crash reporting |
| `renzora_editor` | `cdylib` | The **editor bundle** — statically links ~50 editor-only crates plus the dual-mode `/editor` subcrates as rlibs |
| `renzora_editor_framework` | `rlib` | The editor SDK *implementation* (rlib-only — no dll is emitted) |
| `renzora_native_plugin` | `rlib` | Scans `plugins/`, rebuilds what is stale, loads the rest |
| `renzora_plugin_build` | `rlib` | The compiler driver — reads the SDK manifest and invokes `rustc` directly |

The `renzora_dylib` and `renzora_ember_dylib` crates hold no code of their own: they exist so the host binary, the dlopen'd editor image and every installed plugin share **one** compiled copy of `renzora` and `renzora_ember`, and therefore one translation table, one Console buffer and one theme palette. `bevy` is shared the same way through `bevy_dylib` (`dynamic_linking` + `prefer-dynamic`), so Bevy's `TypeId`s line up across the dlopen boundary too.

> The editor contract (`EditorSelection`, `FieldDef`/`FieldType`/`FieldValue`, the inspector/spawn/toolbar/shortcut registries, `AppEditorExt`, the field macros, `Inspectable`, `post_process`) was **folded into `renzora.dll`** under its `editor` feature. `renzora`'s default features are empty, so a crate that derives `Inspectable` or registers an inspector must depend on `renzora = { ..., features = ["editor"] }`.

## The plugin / ABI model

Every plugin declares itself with **`renzora::add!`**. There is no central list of plugins to edit.

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct GravityPlugin;

impl Plugin for GravityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_gravity);
    }
}

renzora::add!(GravityPlugin);                          // Runtime (default)
renzora::add!(GravityPlugin, Runtime);                 // explicit Runtime
renzora::add!(MyToolPanelPlugin, Editor);              // editor-only
renzora::add!(EarlySetupPlugin, Runtime, priority = -100); // installed earlier
```

> Import the SDK with `use renzora::*;` (or `use renzora::Inspectable;`). There is **no** `renzora::prelude` module.

### What `add!` expands to

**Nothing at all, at the call site.** The macro (`crates/renzora/src/plugin_meta.rs`) exists so the declaration has somewhere to live and a shape a parser can rely on; the actual wiring is written by a build-time generator that reads every `add!` line *as text* and emits two committed files:

- `crates/renzora_runtime/src/plugins.rs` — every `Runtime`-scope plugin
- `crates/renzora_editor/src/plugins.rs` — every `Editor`-scope plugin

Each is an ordinary list of `app.add_plugins(...)` calls in priority order. There is no registry to iterate, no constructor to keep alive against dead-stripping, and no FFI — a named type in a generated list is just a linker symbol.

Because both files are committed, a plain `cargo build` needs no generator run. CI regenerates them and fails on a diff, which is what stops a stale list from shipping.

This replaced an `inventory` registry, and the registry's removal took three dead-strip workarounds with it: a keepalive `build.rs` in each host, and an aggregator whose only job was forcing plugin objects into a lean export.

### Scopes

`PluginScope` is exactly two values and matching is **exact equality** — there is **no "both" scope**:

```rust
pub enum PluginScope { Editor = 0, Runtime = 1 }
```

A feature that needs editor tooling *and* runtime behaviour ships **two** plugins (e.g. `GameUiPlugin` + `GameUiEditorPlugin`).

### Engine plugin install order

For a normal launch, `renzora_runtime::add_engine_plugins(app, is_editor)` inserts the `EditorSession(is_editor)` marker, then installs an ordered foundation:

```
RuntimePlugin → InputPlugin → ScriptingPlugin → PhysicsPlugin
   (+ ViewportStretchPlugin when !is_editor)
```

…and then the generated `Runtime` list. It installs **no editor plugins**: those come from the editor image's own generated list, layered on top after the runtime foundation, and a shipped game simply does not have that image beside it.

Plugins installed from `plugins/` come last, so an installed plugin's systems land where a statically linked one's would.

### The plugin loader

`renzora_native_plugin` runs on the three desktop OSes (a no-op on wasm/mobile). One pass over `<exe>/plugins/`, and for each directory in it:

- **Rebuild anything stale.** A plugin records the content hash of the SDK it was built against; when the engine moves, the stamp stops matching and the plugin is recompiled before the `App` exists. An edit to its source does the same, compared by mtime.
- **Skip anything disabled.** Checked before the directory is touched at all, so a disabled plugin costs no rebuild, no `Library::new`, and no static initializers.
- **Decline anything without the ctor symbol, before mapping it.** "Is this one of ours?" is a byte search for `renzora_native_plugin_ctor` in the file. Deciding after `Library::new` would mean leaking every declined image — a library must never be `LoadLibrary`'d and then `FreeLibrary`'d to answer a question, because that runs its initialisers *and* its destructors, and `FreeLibrary` deadlocks the Windows loader lock against an image that started a thread at map time.
- **Never unload.** Every system a plugin registered is a function pointer into its image, so the handles are `ManuallyDrop` for the life of the process. Disabling a plugin takes effect on the next launch, and the UI says so rather than pretending otherwise.
- **Record what happened.** Loaded, disabled, skipped with a reason, or failed with the compiler's message — all of it into `renzora::PluginInventory`, which is what Settings → Editor → Plugins renders. A panel that scanned for itself would drift from the loader the first time a rule moved.

The export UI reads the same directory through `renzora_native_plugin::installed_for`, which takes each plugin's scope **from the library it built** rather than from its source: the two disagree whenever one was edited without rebuilding, and what ships is the library.

## Rendering and post-processing

Renzora builds on Bevy's PBR/HDR pipeline plus a large family of plugin crates. Camera effects fall into three structural families:

1. **The unified post-process family** — one `RenderComposition` registry (`crates/renzora/src/postprocess.rs`) that runs every active effect as a fullscreen fragment pass, dispatched in one of four phases positioned around Bevy's own tonemapping and anti-aliasing.
2. **Bevy built-in wrappers** that author user-facing settings and route a stock Bevy component onto the camera (bloom → `Bloom`, dof → `DepthOfField`, ssao → `ScreenSpaceAmbientOcclusion`, ssr → `ScreenSpaceReflections`, motion blur, auto-exposure, fog, atmosphere → `Atmosphere`, skybox → `Skybox`, etc.).
3. **Custom multi-pass render-graph crates** for global illumination and transparency (`renzora_lumen`, `renzora_rt`, `renzora_oit`) and for material/mesh sky & water (clouds, night stars, water, lighting).

### The unified post-process pipeline

Each effect registers a type-erased pass into `RenderComposition` under a `(phase, order)` key, and **only runs when its settings component is present**, so inactive effects have zero render-graph overhead.

These effects are [installed plugins](../extending/post-processing.md) under `plugins/`, not engine crates — an effect hot-reloads with its shader while the editor runs. Here is the complete `ascii`:

```rust
use bevy::prelude::*;
use renzora::{post_process, AppEditorExt};

#[post_process(shader = "ascii.wgsl", name = "ASCII", icon = "text-aa")]
pub struct Ascii {
    #[field(min = 2.0, max = 32.0, speed = 0.5, default = 8.0)]
    pub char_size: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01, default = 0.5)]
    pub color_mix: f32,
    #[field(min = 0.5, max = 3.0, speed = 0.01, default = 1.2)]
    pub contrast: f32,
}

#[derive(Default)]
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ascii.wgsl");
        app.add_plugins(renzora::postprocess::PostProcessPlugin::<Ascii>::default());
        app.register_inspectable::<Ascii>();
    }
}

renzora::plugin!(AsciiPlugin, Runtime);
```

`#[post_process]` writes the derives, the `Default` from the `default =` values, the `enabled` field and the padding before it, the `PostProcessEffect` impl, and the inspector section. `PostProcessPlugin<T>` turns that into a real `RenderPassEntry`, sizes the uniform buffer from the component and uploads its bytes each frame.

> `renzora_postprocess` is a re-export shim (`pub use renzora::postprocess::*`). The framework lives **inside `renzora.dll`**, so every effect shares one `RenderComposition` and matching `TypeId`s across the dlopen boundary.

What stays in-tree: the **built-in wrappers** (bloom, DOF, SSAO, SSR, vignette, motion blur, …), which route a stock Bevy component rather than running a pass of their own, and the **multi-pass graph crates** (`renzora_lumen`, `renzora_rt`, `renzora_oit`, `renzora_solari`).

## Scene serialization

Scenes are saved as **RON** (`.ron`). Save and load both live in `renzora_engine/scene_io.rs`.

- **Save** (`save_scene`) builds a `DynamicSceneBuilder` and **denies** runtime/editor-only components — meshes, materials, cameras, Avian physics state, animation runtime state, networking, and `bevy_ui` camera plumbing — then serializes the remaining authored data to RON and writes the file. A scene describes *authored content*, not transient state.
- **Load** (`load_scene`) reads the RON (VFS/rpak first, then disk), deserializes **lossily** (skipping any type not in the registry), prunes orphaned editor-chrome UI entities, and expands nested `SceneInstance` references. The denied runtime components are rebuilt by the normal engine systems once the entities exist.

The startup scene is set in `project.toml` (default `scenes/main.ron`):

```toml
name = "MyProject"
main_scene = "scenes/main.ron"
```

> **Internal inconsistency to know about:** the asset registry's `AssetKind::from_path` maps only the `.scene` extension to `AssetKind::Scene`, so actual `.ron` scenes are indexed as `Other` by the registry — even though the asset-browser UI labels `.ron`/`.scn`/`.scene` as "Scene".

## Asset resolution

Asset handling is four layers:

1. **Asset registry** (`renzora_asset_registry`) — walks the project tree once on load and builds a **metadata-only** index (path, `AssetKind`, size, mtime). It never reads file bytes.
2. **VFS + custom asset reader** (`renzora_engine/vfs.rs`, `asset_reader.rs`) — a virtual filesystem backed by an rpak archive or raw disk.
3. **Import pipeline** (`renzora_import`) — converts non-glTF 3D models to GLB at import time.
4. **Scene save/load** (`scene_io.rs`) — the RON serialization above.

The `EmbeddedAssetReader` resolves every `AssetServer::load` path in this fixed order:

```
absolute path → rpak archive → project-local assets/ → exe-adjacent assets/ → CWD assets/
```

This is what lets the editor hot-reload from a project's `assets/` folder while a packed, shipped build serves the same paths straight out of its `.rpak`.

The packed archive itself is detected by the VFS in its own order at startup:

```
--rpak <path> override → embedded rpak in the exe → adjacent <exe-stem>.rpak
  → platform bundle (Android APK / iOS bundle / WASM injected bytes) → raw filesystem
```

## Crash handling

A panic hook is installed once the session kind is known. The editor writes `~/.renzora/crashes/last_crash.txt` and shows a native dialog; the shipped game silently appends to `<exe_dir>/crash.log`.

## What's next

- [Core Concepts](/docs/r1-alpha8/getting-started/concepts) — the ECS data model and a gentler tour of the one-binary model.
- [Scripting Overview](/docs/r1-alpha8/scripting/overview) — Lua, Rust scripts, and blueprints on top of this architecture.
- [Multiplayer Overview](/docs/r1-alpha8/multiplayer/overview) — how `--server`/`--host` and replication fit in.
- [Exporting](/docs/r1-alpha8/exporting/overview) — turning the editor binary into a shipped game.
