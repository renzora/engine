# Editor / Runtime / Plugin Architecture

Renzora is **one** Bevy 0.18 binary that becomes the editor, the shipped game, or a network server depending on how it launches and what sits beside it.

> **Status: AS-BUILT.** This describes shipped behaviour ("Operation Merge", now complete). The planning narrative that produced it is condensed into the [History appendix](#history-operation-merge).

## One binary, three run modes

The workspace has exactly **one** binary: `renzora_app` (root `Cargo.toml`, `[[bin]] name = "renzora"`, path `src/main.rs`, `default-run = "renzora"`). It is the engine — editor + runtime + server in one — not a project orchestrator.

The binary is **always runtime-shaped**. The root `Cargo.toml` has `default = ["runtime"]`; the only build features are `runtime` and `wasm`. There is **no `editor` compile-time feature and no separate editor binary**. The editor experience is layered on at runtime by a removable cdylib bundle (`renzora_editor`) that sits beside the exe.

> Present the bundle dll beside the exe and the binary *is* the editor. Delete that one file and the same binary is the shipped game.

Which mode runs is decided at runtime in `src/main.rs`:

| Mode | How it's selected | What it does |
|---|---|---|
| **Editor** | Default windowed launch, bundle dll present beside the exe | Boots the runtime, then dlopens the editor bundle on top |
| **Game** | Default windowed launch, no bundle (or `--no-editor`) | The same binary as the exported game — windowed client |
| **Dedicated server** | `--server` | Headless, no GPU/window; `NetworkServerPlugin` |
| **Listen server (host)** | `--host` | Windowed client + server in one process |

```bash
renzora                 # editor if renzora_editor.dll is beside it, else the game
renzora --no-editor     # force game mode even with the bundle present
renzora --server        # headless dedicated server (no window)
renzora --host          # windowed listen server (client + server)
```

> `--host` wins over `--server` if both are passed. A server **or** host launch is **never** an editor session, even if the bundle dll happens to sit beside the exe. The dedicated server is this same `renzora` binary — there is no separate server executable.

### Deciding editor vs game

Two small functions in `src/main.rs` do the whole decision:

- **`editor_bundle_path()`** — looks beside the exe for the bundle cdylib: `renzora_editor.dll` (Windows), `librenzora_editor.so` / `renzora_editor.so` (Linux), or `librenzora_editor.dylib` / `renzora_editor.dylib` (macOS). Returns the first that exists.
- **`editor_session()`** — returns `true` iff the bundle is present **and** neither `--no-editor` nor the `RENZORA_NO_EDITOR` env var is set. The caller additionally excludes server/host launches:

```rust
let host_mode = std::env::args().any(|a| a == "--host");
let server_mode = !host_mode && std::env::args().any(|a| a == "--server");
let is_editor = !server_mode && !host_mode && editor_session();
```

`is_editor` then flows through the whole boot: it selects the crash-file location, decides whether to grab a console on Windows, is handed to `add_engine_plugins`, and gates whether the editor bundle is loaded.

### Server flags

When `--server` or `--host` is present, `load_server_config()` builds a `NetworkConfig` from CLI flags overlaid on `project.toml [network]`:

| Flag | Overrides |
|---|---|
| `--port <u16>` | `[network].port` (default **7636**) |
| `--addr` / `--address <ip>` | `[network].server_addr` |
| `--tick-rate <u16>` | `[network].tick_rate` (default **64**) |
| `--max-clients <u16>` | `[network].max_clients` (default **32**) |

The headless runner and `NetworkServerPlugin` share the resolved tick rate. (`--project <path>` is handled later by the splash plugin, which lives in the editor bundle.)

## Core crate layers

| Crate | Crate type | Role |
|---|---|---|
| `renzora` | `dylib` + `rlib` → **`renzora.dll`** | The SDK / contracts crate: `add!`/`export_plugin_bundle!` macros, `PluginScope`/`StaticPlugin`, GI contract types, the post-process framework, the `runtime_warnings` ring buffer, and (under the `editor` feature) the editor contract registries |
| `renzora_runtime` | rlib | Shared engine library every binary links: `init_app`, `add_default_rendering`, `add_headless_rendering`, `add_engine_plugins` |
| `renzora_engine` | rlib | Editor-free game core: VFS, asset reader, scene IO, autoload, crash reporting |
| `renzora_editor` | `cdylib` → **`renzora_editor.dll`** | The editor **bundle**: statically links ~50 editor-only crates + the dual-mode `/editor` subcrates as rlibs and exposes them through one FFI entry point |
| `renzora_editor_framework` | **rlib only** | The editor SDK *implementation*. Baked into both the binary and the bundle — emits no dll of its own |
| `dynamic_plugin_loader` | rlib | dlopens the bundle + community plugins at startup and hot-reloads new ones dropped into `plugins/` mid-session |

### Shipped files

A desktop editor build ships these files next to the exe:

| File | What it is | Needed by |
|---|---|---|
| `renzora(.exe)` | The engine binary (always runtime-shaped) | always |
| `bevy_dylib-<hash>.{dll,so,dylib}` | The shared Bevy build (render, ECS, math) | editor + game |
| `renzora.{dll,so,dylib}` | The shared Renzora SDK / contracts | editor + game |
| `renzora_editor.{dll,so,dylib}` | The editor **bundle** cdylib (the removable editor) | editor only |
| `plugins/*.{dll,so,dylib}` | dlopen'd distribution plugins | whoever installs them |

A **game export is the same set minus `renzora_editor.*`**. There is no separate "editor SDK" dll: the editor framework is the `renzora_editor_framework` rlib, and the only types that cross the binary ↔ bundle boundary live in `renzora.dll`.

> The old `renzora_editor.dll = "Editor SDK / EditorPanel / egui re-exports"` description is obsolete. `egui`/`bevy_egui` are fully removed (the editor is bevy_ui-native via `renzora_shell` + `renzora_ember`), and `renzora_editor.dll` is the editor *bundle*, not an SDK dll.

## The shared ABI: one `bevy_dylib` + one `renzora.dll`

The whole architecture serves one requirement: **a community plugin built once must load in both the editor and the shipped game.** That only works if the host, the dlopen'd bundle, and dynamic plugins all link an *identical* `bevy_dylib` and `renzora.dll` — same compiled copy, matching `TypeId`s.

That is why `bevy` is built with `dynamic_linking` and `.cargo/config.toml` sets `-C prefer-dynamic`. `prefer-dynamic` applies to crates that emit both rlib + dylib — currently `bevy` (via `dynamic_linking`) and `renzora`. Each ships as a single shared library so statically-linked workspace crates *and* dlopen'd plugins share one compiled copy of the contracts.

The runtime guard is `plugin_bevy_hash()`, emitted by every plugin and the bundle:

```rust
#[no_mangle]
pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
    let id = std::any::TypeId::of::<bevy::ecs::world::World>();
    unsafe { std::mem::transmute(id) }
}
```

The loader computes its own `TypeId::of::<World>()` and **rejects any plugin or bundle whose hash differs** ("incompatible bevy version"). If a plugin linked a different `bevy_dylib`, its `World` would be a distinct type and every component crossing the boundary would mismatch — so the loader refuses to install it.

> `build.rs` also emits `RENZORA_ENGINE_VERSION` and an FNV-1a `RENZORA_BUILD_HASH` (`version + rustc + bevy0.18`) for version reporting, but the *actual* dlopen reject decision is the `plugin_bevy_hash` `World` `TypeId` above.

## Registering a plugin: the `add!` macro

Every Renzora plugin declares itself with `renzora::add!`. The plugin type must implement `Default`.

```rust
use bevy::prelude::*;

#[derive(Default)]
struct MyPlugin;
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) { /* ... */ }
}

renzora::add!(MyPlugin);                          // Runtime (default)
renzora::add!(MyEditorTool, Editor);              // editor only
renzora::add!(MyGameplay, Runtime);               // explicit (same as default)
renzora::add!(MyFoundation, Runtime, priority = -100); // lower = installed earlier
```

> Import from `renzora::*` — there is **no** `renzora::prelude` module.

`add!` expands to two parallel registration paths (`crates/renzora/src/plugin_meta.rs`):

1. **Always, on every platform** — an `inventory::submit!{ StaticPlugin { name, scope, priority, install } }` entry. The shared `renzora.dll` owns the single `inventory::collect!(StaticPlugin)` registry, so every plugin compiled into the binary (or dlopen'd later) self-registers with no manual enumeration.
2. **Only under `#[cfg(feature = "dlopen")]`** (evaluated on the *calling* crate, desktop only) — three `#[no_mangle] extern "C"` exports: `plugin_create() -> *mut dyn Plugin`, `plugin_scope() -> u8`, and `plugin_bevy_hash() -> [u64; 2]`.

### Plugin scopes

```rust
#[repr(u8)]
pub enum PluginScope { Editor = 0, Runtime = 1 }
```

`PluginScope::matches` is **exact equality** — there is **no "both" scope**. `Runtime` plugins run wherever the runtime runs (the editor viewport *and* the shipped game); `Editor` plugins run only in the editor pass (the bundle). A feature that needs editor tooling on top of runtime behaviour ships **two** plugins — e.g. `GameUiPlugin` (Runtime) + `GameUiEditorPlugin` (Editor).

`for_each_static_plugin(host_scope, f)` filters the global inventory by scope and invokes `f` in ascending `priority` order.

### Workspace plugins vs distribution plugins

| Kind | Crate type | `dlopen` feature | Registration |
|---|---|---|---|
| **Workspace plugin** | rlib | off | Statically linked into the host; registers via its `inventory` ctor at process start |
| **Distribution plugin** | cdylib | `dlopen = []`, default-on | dlopen'd from `plugins/` at startup or hot-reload |

The FFI exports are unmangled, so a distribution cdylib may contain **exactly one** `add!` — two would collide on `plugin_create`/`plugin_scope`. Engine crates with multiple plugins (e.g. `renzora_shader::{ShaderPlugin, MaterialPlugin}`) live in the host as rlibs and never enable `dlopen`. Shipping *many* plugins from one cdylib is what the bundle door below is for.

## The editor bundle: `export_plugin_bundle!`

`renzora_editor` is the cdylib that links ~50 editor-only crates as rlibs (with `dlopen` *off*, so none emit the colliding trio) and calls the bundle macro exactly once (`crates/renzora_editor/src/lib.rs`):

```rust
renzora::export_plugin_bundle!(foundation = [
    renzora_asset_registry::AssetRegistryPlugin,
    renzora_editor_framework::RenzoraEditorPlugin,
    renzora_keybindings::KeybindingsPlugin,
]);
```

This emits a single collision-free entry point plus the same ABI guard:

```rust
extern "C" fn plugin_install_scope(app: *mut App, host_scope: u8) -> u32;
extern "C" fn plugin_bevy_hash() -> [u64; 2];
```

`plugin_install_scope`:

1. Installs the ordered `foundation` first (each in `catch_unwind`) — these init the shared registries later plugins read in their own `build()`. The foundation plugins are *not* in the inventory; they're added explicitly and in order.
2. Replays `for_each_static_plugin(scope)` from the **one global inventory**, deduplicated by plugin `name` (a dual-mode editor crate can be linked into both the host and the bundle and submit twice), each install caught individually.
3. Returns the **count of plugins that panicked** (0 = all good). **Nothing unwinds across the `extern "C"` frame.**

> **Global-inventory consequence (load-bearing).** Because the inventory is one shared registry, `plugin_install_scope` replays *every* matching-scope plugin in it, not just the bundle's own. This works only because scopes partition: an editor host drives the bundle with `host_scope = Editor` and installs Editor-scope plugins; the runtime-shaped host installs Runtime-scope plugins itself. A build either statically registers editor plugins **or** ships the bundle — never both, or they double-add and Bevy panics. The shipped runtime host registers **no** editor plugins, so the bundle is the only source of them.

## Engine install order

`renzora_runtime::add_engine_plugins(app, is_editor)` builds the runtime foundation, in this exact order (`crates/renzora_runtime/src/lib.rs`):

```text
EditorSession(is_editor)        // runtime editor-vs-game signal for dual-mode crates
RuntimePlugin                   // renzora_engine
GlobalsPlugin                   // renzora_globals
InputPlugin                     // renzora_input
ScriptingPlugin                 // renzora_scripting (Lua + Rhai)
PhysicsPlugin                   // renzora_physics
ViewportStretchPlugin           // only when !is_editor
for_each_static_plugin(Runtime) // fan out every Runtime-scope plugin in the inventory
```

It installs **no** editor plugins. Editor plugins arrive *only* through the bundle's `plugin_install_scope` with `host_scope = Editor`, loaded *after* this foundation so they layer on top — reproducing the ordering the old (removed) `add_editor_plugins` had.

`src/main.rs` wires the load after `add_engine_plugins`:

```rust
fn load_global_plugins(app: &mut App, is_editor: bool) {
    if is_editor {
        if let Some(bundle) = editor_bundle_path() {
            dynamic_plugin_loader::load_bundle(app, &bundle, true);
        }
    }
    if let Some(dir) = exe_dir() {
        let plugins = dir.join("plugins");
        if plugins.exists() {
            dynamic_plugin_loader::load_plugins(app, &plugins, is_editor);
        }
    }
}
```

## Dynamic loading & hot-reload

`dynamic_plugin_loader` handles the three desktop OSes (no-op on wasm/mobile):

- **`load_bundle(app, path, is_editor)`** — loads exactly one bundle cdylib *by path* (the editor bundle beside the exe). It does **not** directory-scan, so the host's own SDK dylibs (`renzora`, `bevy_dylib`) are never dlopened as plugins. ABI-gated on `plugin_bevy_hash`, then calls `plugin_install_scope(app, Editor)`.
- **`load_plugins(app, dir, is_editor)`** — scans `<exe>/plugins/`, rejects hash mismatches, **skips any cdylib exporting `plugin_install_scope`** (bundles load only beside the exe), reads `plugin_scope`, applies `should_load` (`Editor → is_editor`, `Runtime → always`), then `plugin_create()` + `plugin.build(app)`. The `Library` is kept alive in `DynamicPluginRegistry`. `load_plugins_recursive` is the same but walks the tree.
- **`scan_plugins(dir)`** (export UI) — lists **only Runtime-scope single-plugin cdylibs**, skipping bundles, so the editor is never offered for shipping inside a game.
- **`HotPluginPlugin`** — watches `plugins/` (~1s, on the `Last` schedule) and live-builds newly dropped dlls by swapping the live `World` into a temporary `App`. Main-world plugins activate next frame; render-world plugins can't be wired into the already-running renderer, so they report `HotLoadOutcome::NeedsReload` ("restart to take effect").

## Building & packaging

Local builds use the `.cargo/config.toml` aliases (all on the `dist` profile for consistent plugin hashes):

| Alias | Expands to | Result |
|---|---|---|
| `cargo renzora` | `run --profile dist --workspace --bin renzora` | Editor (builds binary **+** bundle, one `bevy_dylib`) |
| `cargo runtime` | `run --profile dist --bin renzora -- --no-editor` | Game (forces game mode) |
| `cargo server` | `run --profile dist --bin renzora -- --server` | Dedicated server |
| `cargo build-editor` / `build-all` | `build --profile dist --workspace` | Binary + bundle + distribution plugins |
| `cargo build-runtime` | `build --profile dist --bin renzora` | **Lean game binary only** (no bundle, no editor crates — note: *not* `--workspace`) |
| `cargo build-web` | `build … --no-default-features --features wasm --target wasm32-unknown-unknown` | Game-runtime wasm (no wasm editor) |

> The editor build **must** be `--workspace`: that is how Cargo's resolver-2 feature unification gives the binary and the bundle one shared `bevy_dylib`. Build the bundle in isolation (`cargo build -p renzora_editor`) and it links a *separate* static Bevy → a different `World` `TypeId` → `plugin_bevy_hash` mismatch → the loader silently rejects it.

Cross-platform release artifacts are produced by `docker/build-all.sh`, run once per platform inside that platform's `ghcr.io/renzora/<platform>` image (all `FROM` the shared `base`, `docker/base/Dockerfile`, `FROM rust:1.93.0-bookworm` — the single source of the Rust version; there is no `rust-toolchain.toml`). It writes arch-suffixed dirs (`dist/windows-x64`, `dist/linux-x64`, `dist/linux-arm64`, `dist/macos-x64`, `dist/macos-arm64`; macOS/Linux wrap the binary in a `.app`/AppImage `.AppDir`, wasm/mobile drop their artifact flat in the platform dir).

Packaging is the file split made literal:

- **Editor dist** = `renzora(.exe)` + the *exact* `bevy_dylib-<hash>` the host binary imports + `renzora.{dll,so,dylib}` + `renzora_editor.{dll,so,dylib}` (the bundle) + `plugins/`.
- **Game export** = the same, minus the `renzora_editor` bundle dll.

`build-all.sh` copies the bundle beside the exe, and deliberately skips any `renzora_editor.*` (or stale `renzora_editor_bundle.*`) found in `plugins/` so a misplaced bundle can never be loaded twice.

---

## History (Operation Merge)

The current shape is the end state of a refactor internally called **Operation Merge**. The condensed story (kept for context; all steps are shipped):

**The goal.** A community plugin built once must load in both the shipped editor and the shipped runtime — which requires an *identical* `bevy_dylib` for both.

**The trilemma.** You could historically pick two of: one build (ABI fixed), a lean game (no egui riding along), and no big refactor. The blocker was egui: the editor's `bevy_egui` pull diverged the `bevy_dylib` feature set, and ~30 dual-mode crates compiled their egui inspectors *into* the runtime crate, so you couldn't simply strip "the editor half".

**The fixes that landed, in order:**

1. **egui → bevy_ui/ember migration** (done first). With egui gone, the editor draws with the same `bevy_ui`/`renzora_ember` system the game already ships, so a dual-mode crate's inspector costs ~nothing in a game and the `bevy_dylib` divergence shrinks.
2. **The bundle door** (`export_plugin_bundle!`) — additive; lets one cdylib ship many plugins past the one-`add!`-per-cdylib FFI limit.
3. **One build** — the binary became runtime-shaped (editor crates no longer statically linked into it); editor functionality moved entirely into the `renzora_editor` bundle. `build-editor`/`build-all` collapsed to `--workspace`.
4. **Startup load + packaging** — the binary dlopens the bundle from beside the exe when present; the export templates and `docker/build-all.sh` ship the bundle for the editor and omit it for the game.

**Dead ends (do not repeat):**

- Renaming `renzora_editor` → `renzora_editor_api` with a thin facade dll — reverted; "one editor dll" is impossible while contract types must be shared, so the contract simply moved into `renzora.dll` and the framework became an rlib.
- Per-feature alignment of the runtime build to match the editor's `bevy` features — fragile whack-a-mole; replaced by the structural "one build".
