# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), or [Fyrox](https://fyrox.rs/).

## Getting Started

**Prerequisites:** [Rust](https://rustup.rs/)

```bash
cargo install cargo-make
git clone https://github.com/renzora/engine
cd engine
makers run
```

Linux: `sudo apt install libwayland-dev` before building.

### Local builds (host OS only)

These compile for the OS you're on. Mobile crates (`renzora-ios`, `renzora-android`) are excluded automatically -- they're cross-compiled via Docker or their dedicated template scripts.

| Command | What it does |
|---|---|
| `makers run` | Build + run the editor |
| `makers build` | Alias for `makers build-editor` |
| `makers build-editor` | Build the editor + plugins, sync to `dist/<host>/editor/` |
| `makers build-runtime` | Build runtime export template (no editor crates), sync to `dist/<host>/runtime/` |
| `makers build-server` | Build dedicated server (headless), sync to `dist/<host>/server/` |
| `makers build-web` | Build WASM runtime export template → `target/dist/renzora-runtime-web-wasm32.zip` |
| <nobr>`makers build-web-editor`</nobr> | Build WASM editor → `templates/web/` (runs `wasm-bindgen` + `wasm-opt` + brotli; native-only deps make this best-effort) |
| `makers build-android` | Build all Android template APKs (ARM64 + x86_64) |
| <nobr>`makers build-android-arm64`</nobr> | Build Android ARM64 template APK only |
| `makers build-ios` | Build iOS ARM64 template (macOS + Xcode only) |
| `makers clean` | Remove final artifacts for editor + runtime + server (keeps cargo's dep cache) |
| `makers clean-editor` | Same as `clean`, scoped to the editor target |
| `makers clean-runtime` | Same as `clean`, scoped to the runtime target |
| `makers clean-server` | Same as `clean`, scoped to the server target |

### Docker builds (cross-platform)

One container, one bind-mount, one shared `target/` cache. Filter platforms with `--`.

| Command | What it does |
|---|---|
| `makers docker-init` | Set up Docker: build image + create container + start. Idempotent -- skips any step already done. |
| `makers docker-build` | Build all platforms in parallel lanes (auto-runs `docker-init`; fast after first run -- cache persists). Add `-- windows linux` (etc.) to build only a subset. |
| `makers upx` | UPX `--brute` shrink the host binary, SDK dylibs, and every plugin (slow -- minutes per file). Defaults to every platform under `dist/`. Add `-- windows` (etc.) or a path like `-- dist/windows-x64` to scope it. |
| `makers docker-clean` | Wipe the container's build cache |
| `makers docker-destroy` | Remove the container entirely |

Output goes to `dist/<platform>/<target>/` (e.g. `dist/windows-x64/editor/`, `dist/windows-x64/runtime/`).

## Plugin SDK

Plugins are Rust crates that get full Bevy ECS access -- `Commands`, `Query`, `Res`, `ResMut`, `Assets`, everything. No FFI wrappers or translation layers.

The SDK (`renzora` crate) connects plugins to Bevy. It provides the `add!()` macro, editor framework traits (`EditorPanel`, `ThemeManager`), and shared types. It does not re-export engine internals -- plugins interact with engine systems through the ECS.

### Scaffolding

Use the workspace task to create a plugin:

```bash
makers add cool_fx              # engine plugin (Runtime + Editor, baked into binary)
makers add my_panel --editor    # editor-only plugin
makers add my_effect --dylib    # distribution plugin (.dll/.so/.dylib for plugins/)
makers remove cool_fx           # delete one
```

`makers add` creates `crates/renzora_<name>/` with a default skeleton. Static plugins (default and `--editor`) are registered as a dep of `renzora_runtime`; the `crates/*` workspace glob auto-includes the new directory. Distribution plugins (`--dylib`) are built standalone (`crate-type = ["cdylib"]`, `renzora/dlopen` feature) and aren't added to `renzora_runtime` -- they're loaded at runtime by `dynamic_plugin_loader` from the `plugins/` directory next to the binary. Adding a plugin is a single command, no manual edits to the runtime crate.

### Plugin Crate

The scaffold writes this for you:

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct CoolFxPlugin;

impl Plugin for CoolFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(PointLight::default());
}

renzora::add!(CoolFxPlugin);
```

```toml
[package]
name = "renzora_cool_fx"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
```

No `[lib]` section needed -- the crate defaults to `rlib`, gets statically linked into the host binary, and self-registers via an `inventory` ctor at process startup. Same on desktop, iOS staticlib, Android cdylib, and WASM.

### Plugin Scope

Control when your plugin loads:

```rust
renzora::add!(MyPlugin);                          // editor + exported games (default)
renzora::add!(MyPlugin, Editor);                  // editor only
renzora::add!(MyPlugin, Runtime);                 // exported games only
renzora::add!(MyPlugin, EditorAndRuntime, priority = -100); // load earlier
```

The macro emits an `inventory::submit!` block; the runtime iterates the registry once at startup and calls `app.add_plugins(...)` on every entry whose scope matches the host. No central enumeration, no manual `add_plugins` to keep in sync.

### Distribution Plugins

If you want to ship a plugin as a standalone file users can drop into `<renzora>/plugins/`, scaffold it with `--dylib`:

```sh
makers add cool_fx --dylib
```

That scaffold uses `crate-type = ["cdylib"]` and enables the `renzora/dlopen` feature, which makes `renzora::add!` emit unmangled `extern "C"` exports (`plugin_create`, `plugin_scope`, `plugin_bevy_hash`). Build with `cargo build -p renzora_cool_fx --profile dist` and copy the resulting `.dll`/`.so`/`.dylib` into the engine's `plugins/` directory. The host's `dynamic_plugin_loader` finds it at startup, validates the bevy ABI hash, and loads it. The plugin must be built against the same bevy + renzora versions as the host -- otherwise the hash check rejects it.

### Building

Don't build plugins standalone (`cargo build -p my_plugin` from outside `--workspace`) -- it may produce a different `bevy_dylib` hash and the plugin won't load. Stay inside `--workspace`.

## Creating Components

Components are data types that attach to entities and show up in the editor inspector with editable fields.

```rust
use bevy::prelude::*;
use renzora::prelude::*;

#[derive(Component, Default, Reflect, Inspectable)]
#[inspectable(name = "Health", icon = "HEART", category = "gameplay")]
pub struct Health {
    #[field(speed = 1.0, min = 0.0, max = 10000.0)]
    pub current: f32,
    #[field(speed = 1.0, min = 1.0, max = 10000.0)]
    pub max: f32,
    #[field(name = "Shield")]
    pub has_shield: bool,
}

#[derive(Default)]
pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<Health>();
    }
}

renzora::add!(HealthPlugin);
```

Field types are inferred: `f32` renders as a drag slider, `bool` as a checkbox, `String` as a text input, `Vec3` as XYZ fields.

`#[field(...)]` attributes: `speed`, `min`, `max`, `name`, `skip`, `readonly`

`#[inspectable(...)]` attributes: `name`, `icon` (Phosphor icon name), `category`, `type_id`

## Scripting

The engine supports Rhai (all platforms) and Lua (native only) scripting. Components registered with `register_inspectable()` are automatically available to scripts -- no extra setup needed. The component is added to Bevy's ECS and reflection system, so scripts can read and write any field.

```lua
-- get a component field
local hp = get(entity, "Health", "current")

-- set a component field
set(entity, "Health", "current", 50.0)
```

This works for any component from any plugin. If someone publishes a `Sun` plugin with a `SunLight` component, scripts can immediately do `set(entity, "SunLight", "intensity", 2.0)` without the plugin author writing any scripting glue.

## Workspaces and Stable ABI

Rust has no stable ABI, so the engine and its plugins must be compiled together to share types safely. Each target builds with `--workspace` into its own directory (`target/{editor,runtime,server}/`), compiling Bevy once into a shared `bevy_dylib` that everything links against — giving identical `TypeId`s. Separate directories also mean switching targets doesn't invalidate the others' caches.

Distribution plugins are checked at load time: each exports a `plugin_bevy_hash()`, and the loader rejects any whose hash doesn't match the engine's (i.e. built with a different compiler, Bevy version, or profile).

## Supported Platforms

| Platform | Devices |
|----------|---------|
| Windows x64 | Desktop, PCVR (SteamVR, Oculus Link) |
| Linux x64 | Desktop, Steam Deck |
| macOS | Intel + Apple Silicon |
| Web (WASM) | Chrome 113+, Edge 113+, Firefox Nightly |
| Android ARM64 | Phones, tablets, Meta Quest, Pico, HTC Vive Focus |
| iOS | iPhone, iPad |
| Apple TV | Apple TV 4K, Apple TV HD |

## Supported File Formats

| Format | Type |
|--------|------|
| `.glb` / `.gltf` / `.fbx` / `.obj` / `.stl` / `.ply` | 3D models |
| `.ron` | Scene files |
| `.rhai` / `.lua` | Scripts |
| `.blueprint` | Visual script graphs |
| `.material` | Material graphs |
| `.particle` | Particle effects |
| `.png` / `.jpg` / `.hdr` / `.exr` | Textures |
| `.ogg` / `.mp3` / `.wav` / `.flac` | Audio (native only) |
| `.rpak` | Compressed asset archives |

## License

Dual-licensed under MIT or Apache 2.0.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)
