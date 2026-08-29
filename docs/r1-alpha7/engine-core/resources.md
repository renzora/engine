# Resources & State

Resources are the engine's global singletons — one instance of a type, readable from any system — and Renzora puts the ones that must cross the editor/runtime/plugin dylib boundary into a single shared crate so their `TypeId`s always match.

## What a resource is

A resource is exactly Bevy's `Resource`: a single value of a type stored in the `World`, not attached to any entity. Use them for global game state, configuration, subsystem handles, caches, and indexes.

```rust
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct GameState {
    pub score: u32,
    pub level: u32,
    pub paused: bool,
}
```

Insert one with `Default`, with an explicit value, or at runtime through `Commands`:

```rust
app.init_resource::<GameState>();

app.insert_resource(GameState { score: 0, level: 1, paused: false });

fn restart(mut commands: Commands) {
    commands.insert_resource(GameState::default());
    commands.remove_resource::<SomeOtherResource>();
}
```

Access them in systems with `Res<T>` (shared), `ResMut<T>` (mutable), or `Option<Res<T>>` when the resource may not exist:

```rust
fn show_score(state: Res<GameState>) {
    info!("score = {}", state.score);
}

fn add_score(mut state: ResMut<GameState>) {
    state.score += 10;
}

fn maybe_config(cfg: Option<Res<GameState>>) {
    if let Some(cfg) = cfg {
        info!("level {}", cfg.level);
    }
}
```

> This is plain Bevy 0.19 — there is no Renzora-specific resource trait or macro. `#[derive(Resource)]` and the `Res`/`ResMut` system params come straight from `bevy::prelude`.

## The shared-contract pattern

Renzora is **one binary** that decides at runtime whether it is the editor, the shipped game, or a dedicated server, plus a removable `renzora_editor` bundle (a cdylib loaded beside the exe) and any dynamic plugins dropped into `plugins/`. Those are **separate compiled artifacts** that all run in one process.

That creates a hard rule for shared state: a resource only works as a single source of truth if every artifact agrees on its `TypeId`. If two dylibs each compiled their own copy of a type, the `World` would treat them as two unrelated resources. The fix is to define the boundary-crossing types **once**, in the `renzora` SDK crate, which ships as `renzora.dll` (`crate-type = ["dylib", "rlib"]`). The host binary, the dlopen'd editor bundle, and dynamic plugins all link that one compiled copy, so a `Res<EditorSession>` in the game binary and a `ResMut<EffectRouting>` in a plugin point at the same instance.

These are the **contract resources/types** that live in `renzora` for exactly this reason:

| Type | Module | Role |
|---|---|---|
| `EditorSession(bool)` | `renzora` (`core`) | Editor-vs-game flag, set once at startup |
| `CurrentProject` / `ProjectConfig` | `renzora` (`core`) | The open project and its `project.toml` |
| `PlayModeState` | `renzora` (`core`) | Editing / Playing / Paused |
| `EffectRouting` | `renzora` (`core`) | Maps post-process settings entities onto active cameras |
| `LumenDiagState` | `renzora` (`gi`) | GI diagnostics, written in the editor, read by the debugger panel |
| `NetworkBridge`, `ScriptRpcInbox`, `ScriptNetLifecycleInbox`, `ScriptUiInbox` | `renzora` (`core`) | Decoupling inboxes between networking/UI and scripting |
| `ShellPanelRegistry`, `NativePanelIds`, `ShellStatusRegistry` | `renzora` (`core`) | Editor shell panel/status registries (under the `editor` feature) |

> The editor-only contract types (the registries below, plus `EditorSelection`, `FieldDef`/`FieldType`/`FieldValue` and the `Inspectable`/`post_process` macros) are gated behind `renzora`'s `editor` cargo feature. A runtime-only plugin links `renzora` with default features (`[]`) and never sees them; an editor plugin uses `renzora = { ..., features = ["editor"] }`.

## EditorSession — editor vs. game at runtime

There is no compile-time `editor` feature on the engine binary. The same binary is the editor when `renzora_editor.{dll,so,dylib}` sits beside it and the game when that file is deleted. To let the dual-mode crates branch correctly **without** being recompiled, `renzora_runtime::add_engine_plugins` inserts a single flag before any foundation plugin builds:

```rust
// renzora_runtime::add_engine_plugins(app, is_editor)
app.insert_resource(renzora::EditorSession(is_editor));
```

```rust
use bevy::prelude::*;
use renzora::EditorSession;

fn only_in_game(session: Res<EditorSession>) {
    if !session.is_editor() {
        // shipped-game startup path
    }
}
```

`EditorSession(bool)` defaults to `false` (a plain game) when the resource is absent. `RuntimePlugin` reads it to decide whether to run the rpak/project/scene game-startup itself or defer to the editor's splash flow.

## Cross-scene state

There is currently **no engine-provided global key/value store**. The `renzora_globals` crate and its `GlobalStore` were the surviving half of the removed lifecycle graph and were deleted with it; the `global_set` / `global_get` script verbs they backed had already lost their handlers and are gone too. A replacement lifecycle system is being designed.

Until it lands, state that must outlive a scene load goes in your own `Resource` (scene loads despawn entities, never resources), or on an entity in a **global scene**.

### Global scenes

A global scene is an ordinary scene listed in `project.toml`'s `autoload`. It loads *before* the boot scene, and every entity it spawns is tagged `renzora::Persistent` — which the scene-load despawn filter (`Without<Persistent>`) skips. The result is content that survives every subsequent `load_scene()`: one scene for your HUD, one for music, one for networking, rather than rebuilding them per level.

Set them in **Settings → Project → Global Scenes**: a toggle per scene in `scenes/`. The list is ordered, which matters only if two global scenes touch the same thing at boot.

```toml
# project.toml
autoload = ["scenes/ui.bsn", "scenes/music.bsn", "scenes/net.bsn"]
```

Reaching their entities needs no special API — script entity lookup by id is world-wide, not scene-scoped, so a script in level 3 can address a global scene's entity directly:

```lua
set_on("music_player", "AudioSink.volume", 0.5)
```

Three things to know:

- **A global scene's script is the only thing still running during a scene swap.** Everything in the outgoing scene is despawned partway through the load, which is why `on_scene_loaded` / `scene_load_state()` (see [Scripting API](../api/scripting#assets)) are only useful from here. A loading screen has to live in a global scene.
- **They run in the editor too.** A game build loads them at `Startup`; in the editor, Play (and Simulate) loads them and Stop despawns them again, so you can test a global HUD or loading screen without an export. Teardown removes exactly what Play spawned — a `Persistent` marker you applied by hand in the inspector is left alone.
- **They are excluded from scene saves.** `Persistent` entities are live in the world but belong to another scene file, so saving the open scene skips them — otherwise every save would bake in a duplicate copy and the next load would spawn two music players.

## Project & play-mode state

The open project is held in `CurrentProject`, whose `config` is the deserialized `project.toml`:

```rust
#[derive(Resource, Clone, Debug)]
pub struct CurrentProject {
    pub path: PathBuf,        // project root
    pub config: ProjectConfig,
}
```

`ProjectConfig` carries the real `project.toml` fields — note `main_scene` (a flat top-level field, **not** `default_scene`), plus `autoload`, `window`, `viewport`, `rendering`/`rendering_2d`, `audio`, and optional `network`/`editor` sections:

```rust
#[derive(Resource, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,            // e.g. "scenes/main.ron"
    pub editor_last_scene: Option<String>, // editor-only, ignored by exports
    pub editor_open_tabs: Vec<EditorOpenTab>, // editor-only, ignored by exports
    pub autoload: Vec<String>,
    pub audio: AudioConfig,            // the mixer bus graph — shipped, not stripped
    // window / viewport / rendering / network / editor sub-configs ...
}
```

### `[audio]` — the mixer bus graph

`AudioConfig` is the project's mixer, saved so a shipped game has one. Each entry
carries a **`key`** and a **`name`**, and the difference matters:

```toml
[[audio.buses]]
key = "Master"
volume = 1.0

[[audio.buses]]
key = "Bus 1"
name = "Footsteps"      # what the mixer shows
volume = 0.8
panning = -0.25
color = [120, 200, 80]
```

The **key is the routing key** — it is what `AudioPlayer.bus` stores, what a
timeline track's `bus_name` holds, and what scripts name. It is fixed when the
bus is created and never changes. The **name is a label**, shown only in the
Mixer panel and free to edit.

Keeping them apart is what makes renaming safe. When the name *was* the key,
renaming a bus meant finding and re-pointing every `AudioPlayer` and timeline
track aimed at the old one — and it could only ever fix the scene that happened
to be open, so any closed scene's emitters were left routed to a name nothing
answered to. Now a rename touches one string and nothing else.

The four built-in buses keep their contractual keys (`Master`, `Sfx`, `Music`,
`Ambient`) and can be neither renamed nor removed. A custom bus may not take a
key already in use; requested keys are uniquified (`Voice`, `Voice 2`, …).

A project.toml with no `[audio]` section loads the default board, so existing
projects are unaffected. The editor rewrites the section when the mixer changes
(throttled), and export ships it — without it, an exported game boots with only
the four built-ins and every emitter routed to a custom bus falls through to the
SFX fallback: right volume, wrong bus, no error.

`editor_open_tabs` records every document tab that was open when the project was last used (`{ path, kind }` entries, in display order, `kind` one of `scene`/`material`/`particle`/`blueprint`/`script`/`shader`/`other`). The editor rewrites it whenever a pathed tab is added, closed, or reordered and restores the whole tab set on project load; the *active* scene still comes from `editor_last_scene`, which also updates as you switch between scene tabs. Both fields are stripped from exported builds.

`CurrentProject` provides `resolve_path("scenes/foo.ron")`, `main_scene_path()`, `make_relative(..)`, and `save_config()` (writes `project.toml` back).

Play state lives in `PlayModeState`:

```rust
use renzora::{PlayModeState, PlayState};

fn pause_world(mut play: ResMut<PlayModeState>) {
    play.request_pause = true;
}
```

`PlayState` is `Editing`, `Playing`, or `Paused`. The resource exposes helpers (`is_playing`, `is_paused`, `is_in_play_mode`, `is_scripts_running`) and `request_*` flags that the editor consumes next frame. The free function `not_in_play_mode` is a run-condition for editor-only systems.

## Editor registries

The editor extends itself through registry resources, not a panel trait. Plugins add to them through `App` extension methods rather than touching the resources directly. These all require `renzora`'s `editor` feature.

```rust
use bevy::prelude::*;
use renzora::{RenzoraShellExt, NativePanelExt};

fn build(app: &mut App) {
    app.register_shell_panel("my_panel", "My Panel", "gauge", "Tools")
       .register_native_panel("my_panel");
}
```

- `register_shell_panel(id, title, icon, category)` adds metadata to `ShellPanelRegistry` (the shell pre-seeds ~55 panels from its own table). `icon` is a Phosphor icon **name** in kebab-case.
- `register_native_panel(id)` marks the id in `NativePanelIds` so the shell skips its placeholder dispatch — pair it with `renzora_ember`'s `register_panel_content(id, scroll, build_fn)`, which renders the bevy-native content.
- `register_shell_status_item(item)` pushes a per-frame status-bar segment into `ShellStatusRegistry`.

The inspector/spawn/tool/shortcut side comes from `AppEditorExt` (also `editor`-gated): `register_inspector`, `register_inspectable::<T>()`, `register_entity_preset`, `register_scene_starter`, `register_component_icon`, `register_tool`, and `register_shortcut`. Tools and shortcuts registered this way are auto-surfaced in the Command Palette with no extra wiring.

> There is no `EditorPanel` egui trait and no `register_panel` call — egui was removed entirely; the shell is bevy_ui-native and panels are registered through the methods above.

## Engine subsystem resources

The big engine subsystems each expose their state as a resource you can read from a system:

| Resource | Crate | What it holds |
|---|---|---|
| `ScriptEngine` | `renzora_scripting` | The active script backends (`Vec<Box<dyn ScriptBackend>>`, dispatched by file extension) |
| `AssetRegistry` | `renzora_asset_registry` | A metadata-only index (path, `AssetKind`, size, mtime) of every file under the project; rebuilt on project open |
| `NetworkStatus` | `renzora_network` | Connection state, `is_server`, `client_id`, and per-client info |
| `EffectRouting` | `renzora` | `routes: Vec<(Entity, Vec<Entity>)>` mapping post-process settings sources onto target cameras |
| `LumenDiagState` | `renzora` (`gi`) | Per-frame GI diagnostics snapshot |

> `ScriptEngine` is a registry of backends, not a single `lua_state` — scripts dispatch to a backend by extension (`.lua` → Lua, `.rs` → Rust scripts), and a backend arrives from a plugin rather than being compiled in. And several `NetworkStatus` fields (`rtt_ms`, `jitter_ms`, `packet_loss`, `client_id`) are defined but not yet populated by the networking layer, so they currently read as zero/`None`.

### Browsing them: the Resources panel

The editor ships a **Resources** panel (Add-Panel picker → *Debug* → *Resources*) that lists every reflected resource in the running world and lets you read and edit its fields live. It is the resource counterpart of the Inspector: the Inspector draws what is on the selected *entity*, and a resource has no entity to select, so global state had nowhere else to be looked at.

It is master/detail — a searchable list of resource names on top, the selected resource's fields below — because a world holds several hundred resources and only one of them is ever being read at a time. Selecting a row is what walks that resource through reflection; nothing else is inspected, and nothing else costs anything.

Rows are generated from `bevy_reflect`, so a resource needs no editor code to appear:

```rust
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]          // <- this is the whole opt-in
struct Wind {
    speed: f32,
    #[reflect(@0.0f32..=1.0f32)]   // a declared range becomes a clamped drag
    gustiness: f32,
}
```

Named structs, newtypes (`struct Score(i32)`) and bare enums (`enum Mode { .. }`) all draw; a field reflection can describe but not edit shows as a read-only row rather than being dropped, so what you see is the whole resource.

Two limits worth knowing:

- **A resource without `#[reflect(Resource)]` is counted, not listed.** The panel's count reads e.g. `412  (+38 unreflected)`. There is nothing to list them under: naming a component outside the type registry means `ComponentInfo::name()`, which returns a placeholder string unless Bevy's `debug` feature is compiled in, and this workspace does not enable it. Derive `Reflect` and the resource appears.
- **Edits here are not undoable.** Undo stacks are per-document and the active one is almost always the scene's; a global poked from a debug panel does not belong in the scene's edit history.

Resources declared by a C-ABI plugin are a separate case — they are not Rust types this build knows, so reflection cannot see them at all. Those have their own **Plugin Resources** panel, driven by the plugin's field schema.

### The runtime-warnings buffer (the exception)

One piece of shared state is deliberately **not** a resource. The Scene Diagnostics warning feed lives in `renzora::runtime_warnings` as a process-global `static` ring buffer, because it has to be written by the capture layer at `LogPlugin` build time — before the editor bundle is even loaded — and read later from inside the bundle (a different dylib). A `Resource` clone would duplicate across that boundary, so it is hosted in the one shared `renzora.dll` as a static instead:

```rust
use renzora::runtime_warnings::{recent_warnings, CapturedWarning};

fn diagnostics_panel() {
    let warnings: Vec<CapturedWarning> = recent_warnings(); // newest last
    for w in &warnings {
        // w.level, w.target, w.message, w.age()
    }
}
```

It keeps the most recent `MAX_WARNINGS` (200) WARN/ERROR tracing events from anywhere in the engine.

## Local & non-send resources

`Local<T>` is per-system state that persists across frames but is private to one system; it's initialized with `Default::default()` on first run:

```rust
fn tick(mut counter: Local<u32>) {
    *counter += 1;
    if *counter % 60 == 0 {
        info!("60 frames elapsed");
    }
}
```

For types that can't move between threads (raw GPU/audio handles), use a non-send resource — its systems run on the main thread only:

```rust
app.insert_non_send_resource(my_handle);

fn use_handle(handle: NonSend<MyHandle>) {
    // main thread only
}
```

> Non-send resources are rarer than they were: `renzora_audio` used to keep its audio manager as one, because a `cpal::Stream` is `!Send`. The device now lives in the audio *plugin*, so what the engine holds is `AudioLink` — a name and a function pointer — which is an ordinary `Resource`.

## Change detection

Resources support Bevy change detection. Check inside a system, or gate the whole system with a run condition:

```rust
fn on_score_change(state: Res<GameState>) {
    if state.is_changed() {
        info!("score is now {}", state.score);
    }
}

app.add_systems(Update, on_score_change.run_if(resource_changed::<GameState>));
```

This is how reactive editor and UI systems avoid recomputing every frame — `resource_changed::<CurrentProject>` and `resource_changed::<PlayModeState>` are common gates throughout the engine.
