# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), or [Fyrox](https://fyrox.rs/).

## Getting Started

**Prerequisites:** [Docker](https://docs.docker.com/get-docker/), and Rust just to install the CLI.

```bash
cargo install renzora     # installs the `renzora` command
renzora new engine        # scaffold a new project
cd engine
renzora run               # build the editor and launch it (first run is slow)
```

Everything builds inside a container, so Docker handles the rest — no toolchain or system libraries to set up, and the build is identical on every machine. The editor runs on your computer, not in the container.

### Commands

| Command | What it does |
|---|---|
| `renzora new <name>` | Scaffold a new project. |
| `renzora run [editor\|runtime]` | Build for your machine and launch it (editor by default). |
| `renzora build [platforms]` | Cross-build for one or more platforms (no args = all). |
| `renzora test` | Run the test suite. |
| `renzora add <name> [--editor\|--dylib]` | Add a plugin crate. |
| `renzora remove <name>` | Delete a plugin crate. |
| `renzora shell` | Open a shell in the build container. |

Run `renzora --help` for the rest (`init`, `check`, `upx`, `clean`, `destroy`).

Platforms: `windows`, `linux`, `macos`, `wasm`, `android`, `ios`. Builds land in `dist/<platform>/` — the runtime build doubles as a dedicated server (run it with `--server`).

### IDE setup

Want code intelligence? Open the repo in VS Code and **Reopen in Container** — rust-analyzer runs inside the same image, so all you install locally is Docker and VS Code.

## Plugin SDK

Plugins are Rust crates that get full Bevy ECS access -- `Commands`, `Query`, `Res`, `ResMut`, `Assets`, everything. No FFI wrappers or translation layers.

The SDK (`renzora` crate) connects plugins to Bevy. It provides the `add!()` macro, editor framework traits (`EditorPanel`, `ThemeManager`), and shared types. It does not re-export engine internals -- plugins interact with engine systems through the ECS.

### Scaffolding

```bash
renzora add cool_fx              # plugin for the editor and exported games
renzora add my_panel --editor    # editor-only plugin
renzora add my_effect --dylib    # distributable plugin (drop-in .dll/.so/.dylib)
renzora remove cool_fx           # delete one
```

This creates `crates/renzora_<name>/` with a working skeleton and wires it into the build automatically — no manual edits to any other crate.

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

To ship a plugin as a standalone file users can drop into `<renzora>/plugins/`, scaffold it with `--dylib`:

```sh
renzora add cool_fx --dylib
```

Build it and drop the resulting `.dll`/`.so`/`.dylib` into the engine's `plugins/` directory — the engine finds and loads it at startup. It has to be built against the same engine version, or it's rejected at load time.

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
