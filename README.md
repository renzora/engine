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
| `makers docker-run -- <platforms>` | Build a subset, e.g. `makers docker-run -- windows linux` |
| `makers docker-clean` | Wipe the container's build cache |
| `makers docker-destroy` | Remove the container entirely |
| `makers upx` | UPX `--brute` shrink the host binary, SDK dylibs, and every plugin (slow). Pass platform paths after `--` to scope it. |

Pass platforms after `--` to skip the rest. Recognized: `linux`, `windows`, `macos` (= both archs), `macos-x64`, `macos-arm64`, `wasm`, `android` (= both archs), `android-arm64`, `android-x86`, `ios`. Order doesn't matter and unknown names are silently no-ops (typo-safe).

Each build target (editor, runtime, server) is isolated with its own feature flag and target directory. No feature unification, no hash mixing. Switching between local targets is instant after the first build of each.

Output goes to `dist/<platform>/<target>/` (e.g. `dist/windows-x64/editor/`, `dist/windows-x64/runtime/`). Each folder is self-contained with the binary, SDK DLL, shared libraries, and plugins. macOS builds produce `.app` bundles; Linux editor builds produce `.AppImage`.

#### Prerequisites for Docker builds

The image cross-compiles to all platforms from a single `linux/amd64` container. A few one-time setup steps depending on your host:

- **macOS cross-compilation**: Apple's SDK isn't redistributable, so you extract it from your own Xcode install once and drop the tarball at `docker/sdk/MacOSX26.1.sdk.tar.xz`. See `docker/sdk/README.md` for the extraction command.
- **iOS cross-compilation**: same story for iPhoneOS — drop `docker/sdk/iPhoneOS15.5.sdk.tar.xz`. The README links to a community-maintained SDK release if you don't have a Mac handy.
- **Apple Silicon hosts (M1/M2/M3/M4)**: Docker Desktop's default QEMU emulation is flaky with `apt-get` (you'll see random I/O errors mid-install). Enable Rosetta instead: **Docker Desktop → Settings → General → Use Virtualization framework**, then **Features in development → Use Rosetta for x86_64/amd64 emulation**. [OrbStack](https://orbstack.dev) is a lighter alternative that handles this out of the box.
- **Windows / x86_64 Linux hosts**: no extra setup — the image builds natively.

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

### Loading

**Desktop**: workspace plugins are baked into the binary (rlib). Distribution plugins are dlopen'd at startup from:
- `plugins/` -- next to the binary
- `<project>/plugins/` -- project-specific plugins (editor only)

Restart the editor to pick up new distribution plugins.

**Mobile / WASM**: there's no runtime plugin loading (Apple/Google Play forbid it; WASM has no dlopen). All plugins are statically linked at build time, registered through the same `inventory` ctor path. From the plugin author's view, the `renzora::add!` call site is identical to desktop.

### Building

For desktop development, just `makers run` -- workspace builds compile the engine + every plugin together with matching `bevy_dylib` hashes.

For per-target builds: `makers build-editor`, `makers build-runtime`, or `makers build-server`. Plugin DLLs land in `dist/<platform>/<target>/plugins/`.

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

The Docker **image** (`makers docker-build`) is the shared toolchain -- Rust compiler, cross-compilation tools (osxcross for macOS/iOS, xwin for Windows MSVC, Android NDK, Clang 19 with LLD for everything), and system dependencies. The **container** is your build environment with cached compilation artifacts.

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
