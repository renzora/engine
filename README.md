# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), or [Fyrox](https://fyrox.rs/).

## Getting Started

**Prerequisites:** [Rust](https://rustup.rs/), then `cargo install cargo-make`

```bash
git clone https://github.com/renzora/engine
cd engine
makers run
```

Linux: `sudo apt install libwayland-dev` before building.

### Local builds

| Command | What it does |
|---|---|
| `makers run` | Build + run the editor |
| `makers build-editor` | Build the editor + plugins |
| `makers build-runtime` | Build runtime export template + plugins |
| `makers build-server` | Build dedicated server + plugins |

### Docker builds (all platforms)

| Command | What it does |
|---|---|
| `makers docker-build` | Build the Docker image (toolchain -- first time only) |
| `makers docker-create` | Create a persistent build container for this directory |
| `makers docker-run` | Build all platforms (fast after first run -- cache persists) |
| `makers docker-clean` | Wipe the container's build cache |
| `makers docker-destroy` | Remove the container entirely |

Each build target (editor, runtime, server) is isolated with its own feature flag and target directory. No feature unification, no hash mixing. Switching between local targets is instant after the first build of each.

Output goes to `dist/<platform>/<target>/` (e.g. `dist/windows-x64/editor/`, `dist/windows-x64/runtime/`). Each folder is self-contained with the binary, SDK DLL, shared libraries, and plugins. macOS builds produce proper `.app` bundles.

## Architecture

One entry point, three build targets:

- **`src/main.rs`** -- Single binary, feature-gated for editor, runtime, or server.
- **`src/runtime.rs`** -- Shared library. Core engine setup functions used by all targets.

Build targets:

- **`--features editor`** -- Full editor with splash screen, project selection, editor panels, and dynamic plugin loading.
- **`--features runtime`** -- Lean game runtime. No editor UI. Loads runtime-scoped plugins only.
- **`--features server`** -- Headless dedicated server. No window, no rendering, no audio.

Core editor infrastructure (viewport, camera, gizmo, grid, scene, keybindings, console) is statically linked into the binary -- not loaded as plugins. All other editor panels are standalone dylib plugins in `plugins/`.

## Plugin SDK

Plugins are Rust `dylib` crates that get full Bevy ECS access -- `Commands`, `Query`, `Res`, `ResMut`, `Assets`, everything. No FFI wrappers or translation layers.

The SDK (`renzora` crate) connects plugins to Bevy. It provides the `add!()` macro, editor framework traits (`EditorPanel`, `ThemeManager`), and shared types. It does not re-export engine internals -- plugins interact with engine systems through the ECS.

### Writing a Plugin

Add a dependency on the SDK crate and write a standard Bevy plugin:

```rust
use bevy::prelude::*;
use renzora::prelude::*;

#[derive(Default)]
pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(PointLight::default());
}

renzora::add!(MyPlugin);
```

### Plugin Cargo.toml

```toml
[package]
name = "my_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
bevy = { workspace = true }
renzora = { path = "../../crates/renzora" }
```

### Plugin Scope

Control when your plugin loads:

```rust
add!(MyPlugin);                // editor + exported games (default)
add!(MyPlugin, Editor);        // editor only
add!(MyPlugin, Runtime);       // exported games only
```

### Building Plugins

1. Create your plugin crate under `crates/` (or anywhere in the repo)
2. Add it to the `[workspace]` members in the root `Cargo.toml`
3. Run `makers build-editor` (or `build-runtime` / `build-server`)

The plugin DLL appears in `dist/<platform>/<target>/plugins/` with a matching `bevy_dylib` hash.

Do **not** build plugins standalone (`cargo build -p my_plugin`) -- this may produce a different `bevy_dylib` hash and the plugin won't load.

### Loading

The engine loads plugins from two locations on startup (before `app.run()`):
- `plugins/` -- next to the binary
- `<project>/plugins/` -- project-specific plugins (editor only)

If the same plugin exists in both locations, the first one loaded takes priority. Restart the editor to pick up new plugins.

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

Rust does not have a stable ABI. Two Rust binaries compiled separately cannot safely share types across a DLL boundary -- even with the same source code, different compilations can produce different memory layouts, vtable offsets, and `TypeId` values.

Renzora solves this with **isolated workspace builds**. The engine and all plugins are members of the same Cargo workspace. Each target (editor, runtime, server) is built separately with `--workspace` and a single feature flag, into its own target directory (`target/editor/`, `target/runtime/`, `target/server/`). Within each build, Cargo compiles Bevy exactly once into `bevy_dylib`, and both the engine and plugins link against that same shared library. This gives them identical type layouts and `TypeId`s.

Separate target directories mean switching between targets doesn't invalidate the other's cache. After the first build of each target, subsequent builds only recompile what changed.

The catch: **plugins must be built with the same compiler, same Bevy version, and same build profile as the engine.** If any of these differ, the `TypeId`s won't match. The engine checks this at load time -- each plugin exports a `plugin_bevy_hash()` function that returns the `TypeId` of `bevy::ecs::world::World`, and the loader compares it against the engine's own hash. Mismatched plugins are rejected with a warning.

## Per-Project Docker Builds

Docker builds are scoped per directory. Running `makers docker-create` creates a persistent container named after a hash of your directory path. This means:

- Two engine forks in different directories get separate containers with separate build caches
- The container persists between builds -- first `makers docker-run` compiles everything, subsequent runs only recompile what changed
- Each fork produces its own editor, runtime, and server binaries with its own plugin hashes
- No cross-contamination between forks, even if they share the same Docker image

The Docker **image** (`makers docker-build`) is the shared toolchain -- Rust compiler, cross-compilation tools (osxcross, MinGW, Android NDK), and system dependencies. The **container** is your build environment with cached compilation artifacts.

```bash
# In ~/projects/my-rpg-engine/
makers docker-create    # container: renzora-a3f1b2c4
makers docker-run       # builds all platforms for this fork

# In ~/projects/my-racing-engine/
makers docker-create    # container: renzora-7e9d0f12
makers docker-run       # builds all platforms for this fork
```

## Exporting

The editor packages your game for any supported platform. It scans referenced assets, strips editor-only components, compresses everything into an `.rpak`, and bundles it with a pre-built runtime template.

Export templates are built via `makers build-runtime` (current platform) or Docker (`makers docker-run`, all platforms). The editor finds templates in the `runtime/` sibling directory next to its own folder. You can also install templates manually from the export overlay.

### Supported Platforms

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
