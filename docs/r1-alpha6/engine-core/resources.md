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
| `GlobalStore` | `renzora_globals` | Cross-system key/value store (uses `renzora::PinValue`) |
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

## Globals — the cross-system key/value store

`renzora_globals` is the small support crate that backs script and blueprint globals. Its `GlobalsPlugin` is installed as a foundation plugin by `add_engine_plugins`, and it provides one resource, `GlobalStore`, plus a per-key change event.

```rust
use bevy::prelude::*;
use renzora::PinValue;
use renzora_globals::{GlobalStore, GlobalChanged};

fn set_phase(mut globals: ResMut<GlobalStore>) {
    globals.set("phase", PinValue::String("boss".into()));
    globals.set("wave", PinValue::Int(3));
}

fn read_phase(globals: Res<GlobalStore>) {
    if let Some(v) = globals.get("phase") {
        info!("phase = {}", v.as_string());
    }
}
```

Values are stored as `renzora::PinValue` — the same tagged value type used on blueprint pins (`None`, `Float`, `Int`, `Bool`, `String`, `Vec2`, `Vec3`, `Color`, `Entity`). `GlobalStore` exposes `get`, `set`, `has`, `clear`, and `iter`. Whenever a key is written or cleared, the `GlobalsPlugin` drains the dirty set in the `First` schedule and fires a `GlobalChanged { key }` observer trigger once per changed key, so reactive UI and systems can respond without polling.

From scripts these globals are reached through the action verbs `global_set` / `global_get`, e.g. `action("global_set", { key = "score", value = 100 })`.

## Project & play-mode state

The open project is held in `CurrentProject`, whose `config` is the deserialized `project.toml`:

```rust
#[derive(Resource, Clone, Debug)]
pub struct CurrentProject {
    pub path: PathBuf,        // project root
    pub config: ProjectConfig,
}
```

`ProjectConfig` carries the real `project.toml` fields — note `main_scene` (a flat top-level field, **not** `default_scene`), plus `autoload`, `window`, `viewport`, `rendering`/`rendering_2d`, and optional `network`/`editor` sections:

```rust
#[derive(Resource, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,            // e.g. "scenes/main.ron"
    pub editor_last_scene: Option<String>, // editor-only, ignored by exports
    pub autoload: Vec<String>,
    // window / viewport / rendering / network / editor sub-configs ...
}
```

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
| `ScriptEngine` | `renzora_scripting` | The active script backends (`Vec<Box<dyn ScriptBackend>>` — Lua + Rhai, dispatched by file extension) |
| `AssetRegistry` | `renzora_asset_registry` | A metadata-only index (path, `AssetKind`, size, mtime) of every file under the project; rebuilt on project open |
| `NetworkStatus` | `renzora_network` | Connection state, `is_server`, `client_id`, and per-client info |
| `EffectRouting` | `renzora` | `routes: Vec<(Entity, Vec<Entity>)>` mapping post-process settings sources onto target cameras |
| `LumenDiagState` | `renzora` (`gi`) | Per-frame GI diagnostics snapshot |

> `ScriptEngine` is a registry of backends, not a `rhai_engine`/`lua_state` pair — scripts dispatch to a backend by extension (`.lua` → Lua, `.rhai` → Rhai). And several `NetworkStatus` fields (`rtt_ms`, `jitter_ms`, `packet_loss`, `client_id`) are defined but not yet populated by the networking layer, so they currently read as zero/`None`.

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

> `renzora_audio`'s `KiraAudioManager` is a real example of a non-send resource.

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
