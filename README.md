# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), or [Fyrox](https://fyrox.rs/).

## Getting Started

**Install Rust** from [rustup.rs](https://rustup.rs/), then:

```bash
cargo renzora       # build + run the editor
cargo runtime       # build + run the game runtime
cargo server        # build + run the dedicated server
```

Build only (no run):

```bash
cargo build-editor
cargo build-runtime
cargo build-server
cargo build-all     # everything including plugins
```

All commands use the `dist` profile. Don't use bare `cargo run` or `cargo build` -- these produce a different `bevy_dylib` hash and break plugin compatibility.

### Cross-Platform

```bash
cargo build-web         # WASM (WebGPU)
cargo build-android     # Android ARM64
cargo build-ios         # iOS ARM64
cargo build-tvos        # Apple TV
```

### Linux

Wayland dev libraries required: `sudo apt install libwayland-dev`

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

Plugins **must** be built as part of the workspace:

```bash
cargo build-all
```

Do **not** build plugins standalone from their own directory -- this produces a different `bevy_dylib` hash and the plugin won't load.

### Loading

The engine loads plugins from `<project>/plugins/` on startup (before `app.run()`). Restart the editor to pick up new plugins.

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

The editor has an export overlay that packages your game for any supported platform. It scans your project for referenced assets, strips editor-only components, optimizes meshes, compresses everything into an `.rpak`, and bundles it with a pre-built runtime template.

### Export Templates

Templates are pre-built runtime binaries for each target platform. You build them once, then the editor injects your game's assets into the template when you export. Template source code lives in `templates/`.

Rebuild templates when you update Bevy, change engine dependencies, or modify `src/runtime.rs`. You do **not** need to rebuild them when you change game assets, scenes, or scripts.

### Desktop

```bash
cargo make dist-runtime       # game runtime for current OS
cargo make dist-server        # dedicated server (headless, no rendering/audio)
```

Output: `target/templates/renzora-runtime-{os}-{arch}` and `renzora-server-{os}-{arch}`

### Web (WASM)

Requires: `rustup target add wasm32-unknown-unknown`

```bash
cargo make dist-web-runtime   # compile + wasm-bindgen + wasm-opt + brotli + zip
```

Output: `target/dist/renzora-runtime-web-wasm32.zip`

The template zip contains JS glue + wasm binary. At export time, the editor packs your assets into an `.rpak`, generates an `index.html`, and bundles everything into a deployable zip. Works on any static host (GitHub Pages, Netlify, Vercel, etc.).

### Android

Requires: Android Studio (SDK + NDK), `cargo install cargo-ndk`, nightly Rust targets

```bash
rustup target add aarch64-linux-android --toolchain nightly
rustup target add x86_64-linux-android --toolchain nightly    # optional, emulator

cargo make dist-android-arm64     # phones, tablets, Meta Quest, Pico
cargo make dist-android-x86       # emulator
cargo make dist-android-firetv    # Fire TV Stick 4K Max, Fire TV Cube (3rd gen+)
cargo make dist-android-all       # all of the above
```

Output: `target/templates/renzora-runtime-android-{arch}.apk`

At export time, the editor injects `game.rpak` into the APK and signs it automatically (ECDSA P-256, no Android SDK needed on the exporting machine).

### iOS / tvOS

Requires: macOS with Xcode, nightly Rust targets

```bash
rustup target add aarch64-apple-ios --toolchain nightly
rustup target add aarch64-apple-tvos --toolchain nightly      # optional

cargo make dist-ios           # iPhone, iPad
cargo make dist-ios-sim       # iOS Simulator
cargo make dist-tvos          # Apple TV
cargo make dist-tvos-sim      # Apple TV Simulator
```

Output: `target/templates/renzora-runtime-ios-arm64.zip`

At export time, the editor injects `game.rpak` into the app bundle and outputs an `.ipa`. Sign with Xcode or `codesign` for distribution.

### Supported Platforms

| Platform | Template | Devices |
|----------|----------|---------|
| Windows x64 | `dist-runtime` | Desktop, PCVR (SteamVR, Oculus Link) |
| Linux x64 | `dist-runtime` | Desktop, Steam Deck |
| macOS | `dist-runtime` | Intel + Apple Silicon |
| Web | `dist-web-runtime` | Chrome 113+, Edge 113+, Firefox Nightly |
| Android ARM64 | `dist-android-arm64` | Phones, tablets, Meta Quest, Pico, HTC Vive Focus |
| Android x86_64 | `dist-android-x86` | Emulators |
| Fire TV | `dist-android-firetv` | Fire TV Stick 4K Max, Fire TV Cube 3rd gen+ |
| iOS | `dist-ios` | iPhone, iPad |
| Apple TV | `dist-tvos` | Apple TV 4K, Apple TV HD |

## Cargo Features

| Feature | Description | Default |
|---------|-------------|---------|
| `editor` | Full editor UI | Yes |
| `dynamic` | Dynamic linking (faster dev builds) | Yes |
| `native` | File watcher, gamepad, platform backends | Yes |
| `server` | Headless mode for dedicated servers | No |
| `solari` | Raytraced GI, DLSS, meshlet geometry (Vulkan + NVIDIA RTX) | No |

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
