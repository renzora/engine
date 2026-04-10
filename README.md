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

| Command | What it does |
|---|---|
| `makers run` | Build + run the editor |
| `makers build` | Build only (no run) |
| `makers docker-build` | Build Docker image (first time — go make a cup of tea) |
| `makers docker-run` | Build all platforms via Docker |

Output goes to `dist/<platform>/` (e.g. `dist/windows-x64/`, `dist/linux-x64/`). Local builds fill in your platform, Docker fills in the rest.

## Architecture

Three binaries share a common runtime library:

- **`src/editor.rs`** -- Editor. Calls `build_runtime_app()` then adds editor UI plugins.
- **`src/runtime.rs`** -- Shared runtime setup. Registers all core plugins.
- **`src/server.rs`** -- Dedicated server. Headless mode (no window, no rendering, no audio).

## Plugin SDK

Plugins are Rust `dylib` crates that get full Bevy ECS access -- `Commands`, `Query`, `Res`, `ResMut`, `Assets`, everything. No FFI wrappers or translation layers.

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
3. Run `makers build`

That's it. The plugin DLL appears in `dist/plugins/` with a matching `bevy_dylib` hash. Copy it to your project's `plugins/` folder, or test it directly from `dist/`.

Do **not** build plugins standalone (`cargo build -p my_plugin`) -- this may produce a different `bevy_dylib` hash and the plugin won't load.

### Loading

The engine loads plugins from two locations on startup (before `app.run()`):
- `dist/plugins/` -- engine-level plugins (next to the editor binary)
- `<project>/plugins/` -- project-specific plugins

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

Rust does not have a stable ABI. This means two Rust binaries compiled separately cannot safely share types across a DLL boundary -- even if they use the same source code, different compilations can produce different memory layouts, vtable offsets, and `TypeId` values.

Renzora solves this by using a **Cargo workspace**. The engine and all plugins are members of the same workspace, and they all depend on `bevy` via `workspace = true`. When built together (with `cargo build-all`), Cargo compiles Bevy exactly once into `bevy_dylib.dll`, and both the engine and plugins link against that same shared library. This gives them identical type layouts and `TypeId`s, so `Res<Time>`, `Query<&Transform>`, etc. work across the DLL boundary as if everything were statically linked.

The catch: **plugins must be built with the same compiler, same Bevy version, and same build profile as the engine.** If any of these differ, the `TypeId`s won't match. The engine checks this at load time -- each plugin exports a `plugin_bevy_hash()` function that returns the `TypeId` of `bevy::ecs::world::World`, and the loader compares it against the engine's own hash. Mismatched plugins are rejected with a warning.

This is why all build commands use the `dist` profile. Using a different profile (like `dev`) produces different hashes, and plugins built with one profile won't load in an engine built with another.

## Exporting

The editor packages your game for any supported platform. It scans referenced assets, strips editor-only components, compresses everything into an `.rpak`, and bundles it with a pre-built runtime template.

Export templates are built via Docker (`makers docker-run`). The editor uses these templates when exporting — you don't rebuild them unless you update Bevy or engine dependencies.

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
