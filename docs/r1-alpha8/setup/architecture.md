# Architecture

How Renzora is put together: one Bevy 0.20 binary, a removable editor bundle, the `renzora.dll` contract, the manifest-declared plugin model, and the render/scene/asset pipelines.

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
| `renzora` | `rlib` (shared through `renzora_dylib`) | The bottom of the contract: the shared components and editor events, project config and settings, the translation table, the Console buffer, and (under the `editor` feature) the editor contract registries |
| `renzora_*_api` and the other subject layers | `rlib` (shared through `renzora_dylib`) | One crate per subject, above `renzora`; see the table below |
| `renzora_sdk` | `rlib` | The top of the contract: re-exports `renzora` and every subject layer under the one name `renzora`, for plugins and Rust scripts. Engine crates never depend on it |
| `renzora_runtime` | `rlib` | Shared engine library every binary links: `init_app`, `add_default_rendering`, `add_headless_rendering`, `add_engine_plugins` |
| `renzora_engine` | `rlib` | The editor-free game core: VFS, custom asset reader, scene IO, autoload, crash reporting |
| `renzora_editor` | `cdylib` | The **editor bundle** — statically links ~50 editor-only crates plus the dual-mode `/editor` subcrates as rlibs |
| `renzora_editor_framework` | `rlib` | The editor SDK *implementation* (rlib-only — no dll is emitted) |
| `renzora_native_plugin` | `rlib` | Scans `plugins/`, rebuilds what is stale, loads the rest |
| `renzora_plugin_build` | `rlib` | The compiler driver — reads the SDK manifest and invokes `rustc` directly |

The `renzora_dylib` crate holds no code of its own: it exists so the host binary, the dlopen'd editor image and every installed plugin share **one** compiled copy of the contract and, under its `ui` feature (which turns on `renzora_sdk/ui`), `renzora_ember`, and therefore one translation table, one Console buffer, one HTTP queue and one theme palette. `bevy` is shared the same way through `bevy_dylib` (`dynamic_linking` + `prefer-dynamic`), so Bevy's `TypeId`s line up across the dlopen boundary too.

### The contract is a hierarchy

The contract is layered the way Bevy is: `renzora` at the bottom, subject crates above it, and `renzora_sdk` on top re-exporting all of them, as the `bevy` crate does for `bevy_ecs`, `bevy_render` and the rest. It is split because Cargo rebuilds every crate downstream of one that changed: while everything lived in `renzora`, an edit to the HTTP layer rebuilt about 200 crates that never make a request.

On disk the hierarchy is two folders:

- **`crates/api/`** holds the contracts, one `renzora_*_api` crate per job. Engine crates build on these and never on each other. A new layer goes here, where the workspace's `crates/api/*` member glob picks it up.
- **`crates/shared/`** holds what hands the contracts out: `renzora` (the name plugins and Rust scripts import) with `renzora_sdk`, and the shared image `renzora_dylib` that keeps every contract and the UI framework (`renzora::ui`) singular. None of these is a contract of its own.

Everything else in `crates/` is engine, built on top of them. The folders are only places on disk: crate names, and so every `use` line, do not depend on them.

Every layer ends in `_api`, and `renzora` itself holds nothing but re-exports (plus the legacy `add!`). `renzora_text_mesh` is not a layer but a library ember and the text3d plugin share, so it sits in `crates/shared/`. A layer's own proc-macro crate nests inside it, as `crates/api/renzora_editor_api/macros` (`renzora_editor_api_macros`) does.

The layers, and what plugins still call each one:

| Crate | Holds | Plugins see it as |
|-------|-------|-------------------|
| `renzora_lang_api` | The translation table: `t`, `t_or`, `t_args`, language packs, the active language and `LanguageChanged`. Depends on Bevy (for the message), serde and toml | `renzora::lang::t`, `renzora::lang::register_pack_str` |
| `renzora_project_api` | The open project and the process around it: `project.toml` (`ProjectConfig`, `CurrentProject`), editor preferences and `settings_file`, `project_files` change events, `session` (process kind, `DedicatedServer`, the editor's one-shot requests), `play_mode`, `image_formats` and the `GraphicsQuality` tier. Depends on Bevy, serde, toml and the Console layer | `renzora::ProjectConfig`, `renzora::PlayModeState`, `renzora::core::project_files::...` |
| `renzora_scene_api` | The scene vocabulary: shared components and entity tags (`SceneCamera`, `HideInHierarchy`, `SceneEntityId`, `MeshInstanceData`, ...), entity ids (`sanitize_id`, `unique_entity_name`), the shape registry, `RenderResolution` and the project byte loader. Its reflected types keep the `renzora::core::..` type paths saved scenes were written with, through `#[type_path]`, and a test fails if one drifts | `renzora::SceneCamera`, `renzora::core::HideInHierarchy` |
| `renzora_viewport_api` | The viewport vocabulary: render-target state, viewport slots and `ViewportSettings`, the render-layer numbers previews and blits use, visualization modes, `in_three_view`/`in_two_view`. Read by runtime render crates as well as the editor, so it is not gated | `renzora::core::viewport_types::...` |
| `renzora_editor_api` | The editor contract: `EditorSelection`, the inspector registry and `FieldDef`, the `*_field!` macros and the `Inspectable` derive (from its nested `macros` crate), the toolbar, spawn, shortcut and GPU-pass registries, gizmo targets, the timeline bridge, `AppEditorExt`. Editor-only by linking: a crate that ships in games makes it optional behind its own `editor` feature | `renzora::EditorSelection`, `renzora::FieldDef`, `renzora::float_field!` (under `renzora`'s `editor` feature) |
| `renzora_shell_api` | The editor chrome's vocabulary: shell panels, status items and top-bar buttons, `keybindings`, `editor_events`, `inspector_pages`, and the editor's extension points: `undo`, `create_menu`, `asset_actions`, `entity_actions` (formerly `renzora_editor_ext`). Ungated, because ember builds its panels from these registries in shipped games too | `renzora::ShellPanelRegistry`, `renzora::core::keybindings::...`, `renzora::inspector_pages::...`, `renzora::undo`, `renzora::create_menu` |
| `renzora_crash_api` | Panics the engine catches on purpose: `catch`, `panic_will_be_caught`, `CaughtPanic`, so the crash reporter can tell a recovered script or loader panic from a real crash. No dependencies at all | `renzora::core::recoverable::catch` |
| `renzora_log_api` | The Console: `console_info`/`console_warn`/`console_error`, the `clog_*!` macros, the buffer the Console panel drains and the history it keeps. No dependencies at all | `renzora::console_log::console_warn`, `renzora::clog_warn!` |
| `renzora_scripting_api` | The script vocabulary: `ScriptCtx`, hooks, `ScriptExtension`, `ScriptBackend`, commands, the property bridge, input actions, `script!`. Depends on Bevy only | `renzora::ScriptCtx`, `renzora::command`, `renzora::script!`, ... |
| `renzora_net_api` | HTTP requests, their queue and the `Backend` contract | `renzora::net`, `renzora::net_backend` |
| `renzora_network_api` | Multiplayer: the replication components, RPCs, ownership and session queries, `NetworkStatus` | `renzora::network` |
| `renzora_audio_api` | The audio vocabulary and the audio backend contract | `renzora::audio`, `renzora::audio_backend` |
| `renzora_diagnostics_api` | Profiling, probes, the load timeline, runtime warnings, content problems | `renzora::profiling`, `renzora::runtime_warnings`, ... |
| `renzora_platform_api` | Version, the Rust toolchain, child processes, DLSS runtime, auth, plugin inventory and updates, the native `plugin!` macro and its ABI record (`native_plugin.rs`), the project index, streaming | `renzora::version`, `renzora::core::...` |
| `renzora_animation_api` | Animation clips, bone styles, sprite animation | `renzora::AnimClip`, `renzora::bone_style`, ... |
| `renzora_save_api` | Save games | `renzora::save` |
| `renzora_bus_api` | Named events and services between plugins | `renzora::bus` |
| `renzora_physics_api` | The physics contract, vehicles, scene queries | `renzora::physics`, `renzora::vehicle`, `renzora::scene_query` |
| `renzora_world_api` | World environment, wind, sun, splines | `renzora::WorldEnvironment`, `renzora::Sun`, ... |
| `renzora_render_api` | The GPU capability flags, `SolariLit` and the ray tracer's stand-in contracts (`gi`), auto exposure, post effects, the post-process framework (`postprocess` feature), the grid (`grid` feature), the blockout grid texture | `renzora::SolariLit`, `renzora::PostEffects`, `renzora::postprocess`, ... |
| `renzora_solari_api` | Solari's settings (`SolariGi` and its enums), DLSS modes and support, the ray tracing diagnostics. Depends on Bevy only; its own crate because it changes far more often than the rest of rendering and only six crates read it | `renzora::SolariGi`, `renzora::DlssMode`, `renzora::gi::...` |
| `renzora_material_api` | `MaterialRef`, the material node catalogue, the node-graph vocabulary. Sits above `renzora_render_api` | `renzora::MaterialRef`, `renzora::GraphComment`, ... |
| `renzora_shader_api` | `wgsl` (parse and validate) and `shader_source` (shader text in any dialect to a Bevy `Shader`). Depends on Bevy only, so a crate that just builds shaders, like the UI toolkit, does not sit under the material and render layers | `renzora::wgsl`, `renzora::shader_source` |
| `renzora_text_mesh` | SDF text meshes (optional, `text_mesh` feature of the sdk) | `renzora::text_mesh` |

- **Engine crates depend on the layers they use**: `renzora` for core types, plus each subject crate whose names they use. Never on `renzora_sdk`, which would bring the cascade back.
- **Depend on the lowest crate that has the name.** A crate with many dependents (`renzora_ember` has about 60) passes every edit below it on to all of them. Ember takes `Request` from `renzora_net_api`, not from `renzora_net` (the frame pump that only the runtime adds), and `shader_source` from `renzora_shader_api`, not from the material layer.
- **A shader embedded by a layer has that layer's crate name in its path** (`embedded://renzora_render_api/postprocess_copy.wesl`). Where a plugin may still load the old path, the layer registers the file under both.
- **Plugins and Rust scripts see one `renzora`**, which is `renzora_sdk`: through `renzora_dylib` when built against the staged SDK, and by a repointed dependency when a lean export compiles them in. A subject's old path keeps working for them (`renzora::net::Request`), so moving a subject out breaks no plugin.
- **`renzora_dylib` embeds every layer**, which is what keeps each layer's statics (the HTTP queue, the translation table) singular across the editor, the runtime and every plugin.
- **`renzora` keeps its name** because it appears in the type path of every component a scene saves (`renzora::core::...`). Moving a type that scenes save into a subject crate changes its type path; the scene loader falls back to the type's short name when the full path is not registered, so a move needs no alias, while a rename still does (see [Scene serialization](#scene-serialization)).

To move a subject out: create the crate above `renzora` (it must not be depended on by `renzora`), add one `pub use` line for it to `renzora_sdk`, and point the engine crates that use it at the new crate.

> The editor contract (`EditorSelection`, `FieldDef`/`FieldType`/`FieldValue`, the inspector/spawn/toolbar/shortcut registries, `AppEditorExt`, the field macros, `Inspectable`, `post_process`) was **folded into `renzora.dll`** under its `editor` feature. `renzora`'s default features are empty, so a crate that derives `Inspectable` or registers an inspector must depend on `renzora = { ..., features = ["editor"] }`.

## The plugin / ABI model

Every engine plugin declares itself with a **`[[package.metadata.renzora.plugin]]`** table in its crate's `Cargo.toml`. There is no central list of plugins to edit.

```rust
// src/lib.rs
use bevy::prelude::*;

#[derive(Default)]
pub struct GravityPlugin;

impl Plugin for GravityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_gravity);
    }
}
```

```toml
# Cargo.toml, one table per plugin
[[package.metadata.renzora.plugin]]
type = "GravityPlugin"                 # Runtime (default)

[[package.metadata.renzora.plugin]]
type = "panel::MyToolPanelPlugin"      # a path from the crate root
scope = "Editor"                       # editor-only

[[package.metadata.renzora.plugin]]
type = "EarlySetupPlugin"
priority = -100                        # installed earlier
```

> Import the SDK with `use renzora::*;` (or `use renzora::Inspectable;`). There is **no** `renzora::prelude` module.

### What a plugin table becomes

**Nothing, in the crate itself.** The table is configuration read only by the generator (`cargo renzora sync`, `manifest_plugins` in `xtask/src/sync.rs`), never code the crate compiles, so a crate needs no contract dependency just to be a plugin. The parser is strict: an unknown key, an unknown scope or a table without `type` is an error. Every module on the `type` path must be `pub`; a `#[cfg(feature = "x")] pub mod y;` gate on it is detected and reproduced. The generator emits two committed files:

- `crates/renzora_runtime/src/plugins.rs` — every `Runtime`-scope plugin
- `crates/renzora_editor/src/plugins.rs` — every `Editor`-scope plugin

Each is an ordinary list of `app.add_plugins(...)` calls in priority order. There is no registry to iterate, no constructor to keep alive against dead-stripping, and no FFI — a named type in a generated list is just a linker symbol. The type must implement `Plugin + Default`, so a wrong type name or a missing `Default` impl is a compile error in the generated `plugins.rs`, not in the crate's own source.

Because both files are committed, a plain `cargo build` needs no generator run. CI regenerates them and fails on a diff, which is what stops a stale list from shipping.

This replaced an `inventory` registry, and the registry's removal took three dead-strip workarounds with it: a keepalive `build.rs` in each host, and an aggregator whose only job was forcing plugin objects into a lean export.

### Scopes

A plugin table's `scope` is exactly one of two values, `Runtime` (the default) or `Editor`, and there is **no "both" scope**. Native plugins have the same two, as `renzora_platform_api::NativePluginScope`:

```rust
pub enum NativePluginScope { Editor = 0, Runtime = 1 }
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

1. **Shader-file effects and render passes**: one `RenderComposition` registry (`crates/shared/renzora/src/postprocess.rs`) of passes, dispatched in one of four phases positioned around Bevy's own tonemapping and anti-aliasing. The Post Process component's effects run as one of those passes.
2. **Bevy built-in wrappers** that author user-facing settings and route a stock Bevy component onto the camera (bloom → `Bloom`, dof → `DepthOfField`, ssao → `ScreenSpaceAmbientOcclusion`, ssr → `ScreenSpaceReflections`, motion blur, auto-exposure, fog, atmosphere → `Atmosphere`, skybox → `Skybox`, etc.).
3. **Custom multi-pass render-graph crates** for transparency (`renzora_oit`) and for material/mesh sky & water (clouds, night stars, water, lighting).

### Render passes and shader-file effects

Each pass registers into `RenderComposition` under a `(phase, order)` key and skips any view without its own component, so an inactive one costs nothing.

A fullscreen effect is not a pass of its own: it is a `.wesl` file that declares its sliders in comments, added to a **Post Process** component. `renzora_postprocess` reads each file's settings struct with naga, packs the buffer to match, and runs the whole list as one pass in `LdrPost`, so adding an effect to a project compiles nothing. See [Post-Processing Effects](../extending/post-processing.md) for the file format.

> The framework lives **inside `renzora.dll`** (`renzora::postprocess`), so every effect shares one `RenderComposition` and matching `TypeId`s across the dlopen boundary. The `renzora_postprocess` crate is the shader-file effects that run in one of its phases.

What stays in-tree: the **built-in wrappers** (bloom, DOF, SSAO, SSR, vignette, motion blur, …), which route a stock Bevy component rather than running a pass of their own, and the **multi-pass graph crates** (`renzora_oit`, `renzora_solari`).

## Scene serialization

Scenes are saved as **RON** (`.ron`). Save and load both live in `renzora_engine/scene_io.rs`.

- **Save** (`save_scene`) builds a `DynamicSceneBuilder` and **denies** runtime/editor-only components (meshes, materials, cameras, the physics engine's own state, animation runtime state, networking, and `bevy_ui` camera plumbing), then serializes the remaining authored data to RON and writes the file. A scene describes *authored content*, not transient state.
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

When the GPU device is lost (a driver reset, or the GPU hanging long enough for the operating system to reset it), the renderer cannot carry on: everything it uploaded belonged to the old device. The editor saves the open scene to `<project>/.renzora/recovery/device_lost_<time>.bsn`, never over the scene file itself, writes a crash report naming the cause and the recovery file, and restarts; the report shows on the next launch. A shipped game writes its report and restarts. A second device loss within a minute of a restart exits instead, so a scene that loses the device as it loads cannot restart forever.

In the editor, a panic inside a system, command or observer is not a crash. Bevy catches it and passes it to the world's `FallbackErrorHandler`, and the editor installs one (in the main world and the render world) that logs it to the Console and keeps running: one broken plugin or script system no longer takes the editor down. The first failure of each system is logged with its message and location, then the count at 10, 100, 1000 and so on. An error a system returns without a severity, which Bevy also treats as a panic, is logged the same way.

The panic hook runs before Bevy's catch, so on a thread that runs systems it holds the report back instead of writing it, and the handler claims it once Bevy has caught the panic. A panic nothing catches unwinds out of `App::run`, and `main` then saves and shows the held report as a normal crash. A shipped game keeps Bevy's default handler, so a panicking system still ends the game and writes `crash.log`.

## What's next

- [Core Concepts](/docs/r1-alpha8/getting-started/concepts) — the ECS data model and a gentler tour of the one-binary model.
- [Scripting Overview](/docs/r1-alpha8/scripting/overview): Lua and Rust scripts on top of this architecture.
- [Multiplayer Overview](/docs/r1-alpha8/multiplayer/overview) — how `--server`/`--host` and replication fit in.
- [Exporting](/docs/r1-alpha8/exporting/overview) — turning the editor binary into a shipped game.
