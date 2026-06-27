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
| `renzora` | `renzora.dll` (`dylib` + `rlib`) | The SDK / "contracts" crate — the `add!`/`export_plugin_bundle!` macros, `PluginScope`/`StaticPlugin`, the GI contract types, the post-process framework, the `runtime_warnings` ring buffer, and (under the `editor` feature) the editor contract registries |
| `renzora_runtime` | `rlib` | Shared engine library every binary links: `init_app`, `add_default_rendering`, `add_headless_rendering`, `add_engine_plugins` |
| `renzora_engine` | `rlib` | The editor-free game core: VFS, custom asset reader, scene IO, autoload, crash reporting |
| `renzora_editor` | `cdylib` | The **editor bundle** — statically links ~50 editor-only crates plus the dual-mode `/editor` subcrates as rlibs |
| `renzora_editor_framework` | `rlib` | The editor SDK *implementation* (rlib-only — no dll is emitted) |
| `dynamic_plugin_loader` | `rlib` | dlopens plugins at startup and hot-reloads ones dropped into `plugins/` mid-session |

Shipping `renzora` as a shared `renzora.dll` means the host binary, the dlopen'd editor bundle, and every dynamic plugin all share **one** compiled copy of the SDK and therefore matching `TypeId`s. `bevy` itself is shared the same way (`bevy_dylib`, `dynamic_linking` + `prefer-dynamic`), so there is one `bevy_dylib` and Bevy's `TypeId`s line up across the dlopen boundary.

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

The macro (`crates/renzora/src/plugin_meta.rs`) emits **two parallel registration paths**:

1. **Always, on every platform** — an `inventory::submit!` of a `StaticPlugin { name, scope, priority, install }`. The `install` closure calls `app.add_plugins(<T as Default>::default())`. These are collected with `inventory::collect!(StaticPlugin)` into one global registry.
2. **Only under `#[cfg(all(feature = "dlopen", not(ios/android/wasm)))]`** on the *calling* crate — three `#[no_mangle] extern "C"` exports so the loader can dlopen a standalone `.dll`/`.so`/`.dylib`:

```rust
#[no_mangle] pub extern "C" fn plugin_create() -> *mut dyn Plugin { /* Box::into_raw(...) */ }
#[no_mangle] pub extern "C" fn plugin_scope()  -> u8            { /* Editor=0 | Runtime=1 */ }
#[no_mangle] pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
    // transmute(TypeId::of::<bevy::ecs::world::World>()) — the cross-dylib ABI guard
}
```

`plugin_bevy_hash` is the **ABI guard**: it is the transmuted `TypeId` of Bevy's `World`. The loader compares a plugin's hash to its own and rejects any mismatch, so a plugin built against a different Bevy/SDK can never be loaded into the running world.

### Scopes

`PluginScope` is exactly two values and matching is **exact equality** — there is **no "both" scope**:

```rust
pub enum PluginScope { Editor = 0, Runtime = 1 }
```

A feature that needs editor tooling *and* runtime behaviour ships **two** plugins (e.g. `GameUiPlugin` + `GameUiEditorPlugin`). `for_each_static_plugin(host_scope, f)` filters the global inventory by scope and runs `f` in priority order.

### Two kinds of plugin

| Kind | Crate type | How it loads |
|------|------------|--------------|
| **Workspace plugin** | `rlib` | Statically linked into the binary/bundle; registers through its inventory constructor at process start. `dlopen` is off, so the FFI symbols are not emitted and can't collide |
| **Distribution plugin** | `cdylib` with a default-on `dlopen = []` feature | dlopen'd at startup, or hot-loaded when dropped into `<exe>/plugins/`. Because the FFI symbols are unmangled, a cdylib may contain **exactly one** `add!` |

### The editor bundle: `export_plugin_bundle!`

A bundle is a cdylib that statically links *many* plugin crates as rlibs (so each runs only its collision-safe inventory constructor) and calls `export_plugin_bundle!` exactly once. `crates/renzora_editor/src/lib.rs` does:

```rust
renzora::export_plugin_bundle!(foundation = [
    renzora_asset_registry::AssetRegistryPlugin,
    renzora_editor_framework::RenzoraEditorPlugin,
    renzora_keybindings::KeybindingsPlugin,
]);
```

This emits a single `extern "C" fn plugin_install_scope(*mut App, host_scope: u8) -> u32` (plus `plugin_bevy_hash`). At call time it installs the ordered `foundation` first, then replays `for_each_static_plugin(Editor)` from the **one global inventory**, dedup'd by name. Every install runs inside `catch_unwind`, and the function returns the count of plugins that panicked — **nothing ever unwinds across the FFI boundary**.

### Engine plugin install order

For a normal launch, `renzora_runtime::add_engine_plugins(app, is_editor)` inserts the `EditorSession(is_editor)` marker, then installs an ordered foundation:

```
RuntimePlugin → GlobalsPlugin → InputPlugin → ScriptingPlugin → PhysicsPlugin
   (+ ViewportStretchPlugin when !is_editor)
```

…and then fans out every `Runtime`-scope plugin from the inventory. It installs **no editor plugins** — those arrive only via the bundle's `plugin_install_scope` with `host_scope = Editor`, layered on top after the runtime foundation.

### The dynamic loader

`dynamic_plugin_loader` runs on the three desktop OSes (a no-op on wasm/mobile). It scans `<exe>/plugins/`, **rejects hash mismatches**, and:

- **Skips any cdylib that exports `plugin_install_scope`** — bundles load only from beside the exe, never from `plugins/`, so the editor can't be shipped *inside* a game.
- Reads `plugin_scope`, gates `should_load` (Editor → only in an editor session; Runtime → always), then calls `plugin_create` and `plugin.build(app)`, keeping the `Library` alive in the `DynamicPluginRegistry`.
- `HotPluginPlugin` watches `plugins/` about once a second on the `Last` schedule and live-builds newly dropped dlls into the running world; render-world plugins report `NeedsReload` ("restart to take effect").
- The export UI's `scan_plugins` lists **only** Runtime-scope single-plugin cdylibs.

## Rendering and post-processing

Renzora builds on Bevy's PBR/HDR pipeline plus a large family of plugin crates. Camera effects fall into three structural families:

1. **The unified post-process family** — a single `UnifiedPostProcessNode` (`crates/renzora/src/postprocess.rs`) that runs every active effect as a fullscreen fragment pass between `Node3d::Tonemapping` and `Node3d::EndMainPassPostProcessing`.
2. **Bevy built-in wrappers** that author user-facing settings and route a stock Bevy component onto the camera (bloom → `Bloom`, dof → `DepthOfField`, ssao → `ScreenSpaceAmbientOcclusion`, ssr → `ScreenSpaceReflections`, motion blur, auto-exposure, fog, atmosphere → `Atmosphere`, skybox → `Skybox`, etc.).
3. **Custom multi-pass render-graph crates** for global illumination and transparency (`renzora_lumen`, `renzora_rt`, `renzora_oit`) and for material/mesh sky & water (clouds, night stars, water, lighting).

### The unified post-process pipeline

Each effect registers a type-erased handler and **only runs when its settings component is present on the camera**, so inactive effects have zero render-graph overhead. Settings authored on any entity are proxied onto the active cameras through the `EffectRouting` table (`renzora/src/core/mod.rs`).

The public trait is small — only `fragment_shader()` is required:

```rust
pub trait PostProcessEffect:
    Component + ExtractComponent + Clone + Copy + ShaderType + WriteInto + Default + 'static
{
    fn fragment_shader() -> ShaderRef;
    // has_extra_texture / extra_texture_is_snapshot / freeze_snapshot have defaults
}
```

Most effects don't implement it by hand — they use the **`#[post_process]`** attribute macro. Here is the complete `renzora_ascii` crate:

```rust
use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "ascii.wgsl", name = "ASCII", icon = "TEXT_AA")]
pub struct AsciiSettings {
    #[field(speed = 0.5, min = 2.0, max = 32.0, default = 8.0)]
    pub char_size: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub color_mix: f32,
    #[field(speed = 0.01, min = 0.5, max = 3.0, default = 1.2)]
    pub contrast: f32,
}

#[derive(Default)]
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ascii.wgsl");
        app.register_type::<AsciiSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<AsciiSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<AsciiSettings>();
    }
}

renzora::add!(AsciiPlugin);
```

The macro auto-adds an `enabled: f32` field plus serde-skipped padding to a 16-byte / two-`vec4` alignment, derives `Component`/`Reflect`/`Serialize`/`ShaderType`/`ExtractComponent` (filtered `With<Camera3d>`), implements `PostProcessEffect::fragment_shader()`, and generates the editor `InspectorEntry`.

> `renzora_postprocess` is now just a re-export shim (`pub use renzora::postprocess::*`). The framework lives **inside `renzora.dll`**, so all of the effect crates share one `PostProcessRegistry` and matching `TypeId`s across the dlopen boundary.

Around **53 effect crates** flow through this pipeline: ~45 use the `#[post_process]` macro, and 8 implement `PostProcessEffect` by hand because they need a custom bind-group layout, an extra texture, or a two-frame snapshot (vignette, outline, gaussian_blur, god_rays, edge_glow, palette_quantization, underwater, screen_transition).

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

- [Core Concepts](/docs/r1-alpha5/getting-started/concepts) — the ECS data model and a gentler tour of the one-binary model.
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — Lua, Rhai, and blueprints on top of this architecture.
- [Multiplayer Overview](/docs/r1-alpha5/multiplayer/overview) — how `--server`/`--host` and replication fit in.
- [Exporting](/docs/r1-alpha5/exporting/overview) — turning the editor binary into a shipped game.
