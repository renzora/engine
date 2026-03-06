# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** This engine is in early alpha. You will encounter bugs, incomplete features, and unexpected behavior. APIs and file formats may change without notice between versions.

## Table of Contents

1. [Documentation](#documentation)
2. [Prerequisites](#prerequisites)
3. [Building & Running](#building--running)
4. [Creating Extensions](#creating-extensions)
6. [Creating Components](#creating-components)
7. [Creating Post-Process Effects](#creating-post-process-effects)
8. [Dynamic Plugins (DLL)](#dynamic-plugins-dll)
9. [Exporting](#exporting)
10. [Building Android Runtime](#building-android-runtime)
11. [Cargo Features](#cargo-features)
12. [Supported File Formats](#supported-file-formats)
13. [Testing](#testing)
14. [Troubleshooting](#troubleshooting)
15. [License](#license)

## Documentation

| Guide | Description |
|-------|-------------|
| **[Engine User Guide](docs/ENGINE_USER_GUIDE.md)** | Viewport controls, keyboard shortcuts, transform gizmos, terrain editing, scene management, components |
| **[Physics Guide](docs/PHYSICS_GUIDE.md)** | Rigid bodies, colliders, cloth simulation, debug panels, stress testing, scripting integration |
| **[Audio Guide](docs/AUDIO_GUIDE.md)** | Kira 0.12 audio system: mixer, spatial audio, emitter/listener components, scripting API |
| **[Console Commands](docs/CONSOLE_COMMANDS.md)** | Built-in `/` commands for the console |
| **[Scripting API Reference](docs/scripting-api.md)** | Full reference for the Rhai scripting API |
| **[VR Guide](docs/VR_GUIDE.md)** | OpenXR VR/XR system: headset setup, controllers, hand tracking, mixed reality |
| **[Plugin Development](docs/PLUGIN_DEVELOPMENT.md)** | Architecture, API reference, UI creation, hot reload, testing |
| **[Contributing Guide](CONTRIBUTING.md)** | Guidelines for submitting issues and pull requests |

## Prerequisites

1. **Install Rust** from [rustup.rs](https://rustup.rs/)
2. Windows 10/11, Linux, or macOS
3. **Linux only:** Wayland dev libraries -- `sudo apt install libwayland-dev` (Debian/Ubuntu)

### Solari / Ray Tracing (Optional)

The `solari` feature enables raytraced global illumination, DLSS Ray Reconstruction, and meshlet virtual geometry. The engine builds and runs fine without it.

| | Windows | Linux | macOS |
|---|---|---|---|
| Compile | Yes | Yes | No |
| Runtime | NVIDIA RTX only | NVIDIA RTX only | No |

#### 1. Vulkan SDK (1.3.x or newer)

Download from [vulkan.lunarg.com](https://vulkan.lunarg.com/sdk/home).

- **Windows:** The installer sets `VULKAN_SDK` automatically.
- **Linux:** `sudo apt install vulkan-sdk` or use the LunarG tarball and set `VULKAN_SDK`.

#### 2. DLSS SDK (v310.4.0)

```bash
git clone --branch v310.4.0 https://github.com/NVIDIA/DLSS.git C:\DLSS_SDK   # Windows
git clone --branch v310.4.0 https://github.com/NVIDIA/DLSS.git ~/DLSS_SDK     # Linux
```

Set the `DLSS_SDK` environment variable to the cloned directory.

### Faster Linking (Recommended)

**Windows:**
```bash
rustup component add llvm-tools-preview
```

**Linux:**
```bash
sudo apt install lld clang
```

## Building & Running

Renzora uses **cargo aliases** so you never need to remember `--bin` or `--no-default-features`.

### Quick Start

```bash
cargo renzora                # build + run the editor
cargo runtime                # build + run the runtime (no editor)
```

### Build Only (no run)

```bash
cargo build-editor           # build editor binary
cargo build-runtime          # build runtime binary
```

### Release Builds

```bash
cargo release-editor         # optimized editor
cargo release-runtime        # optimized runtime
cargo dist-runtime           # max optimized runtime (fat LTO, stripped)
```

### Running the Runtime with a Project

```bash
# PowerShell
& "./target/debug/renzora-runtime.exe" --project "C:\Users\you\Documents\my_game"

# Bash / Linux / macOS
./target/debug/renzora-runtime --project ~/Documents/my_game
```

The runtime also looks for `project.toml` in the current directory if `--project` is not specified.

### Architecture

The engine has two binaries that share a common runtime library:

- **`src/editor.rs`** -- Editor binary. Calls `build_runtime_app()` then adds editor plugins (UI, viewport, inspector, etc.) behind `#[cfg(feature = "editor")]`.
- **`src/runtime.rs`** -- Shared runtime setup. Registers `DefaultPlugins`, `RuntimePlugin`, all post-process effects, environment plugins (skybox, clouds, lighting).

Both binaries (`renzora` and `renzora-runtime`) use the same runtime core. The `editor` feature controls whether the editor UI is included.

## Creating Extensions

Extensions add panels, layouts, and UI to the editor. See `crates/editor/renzora_test_extension/` for a complete example.

### 1. Create the Crate

```toml
# crates/editor/my_extension/Cargo.toml
[package]
name = "my_extension"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { version = "0.18" }
bevy_egui = "0.39"
egui-phosphor = { version = "0.11", features = ["regular"] }
renzora_editor = { path = "../renzora_editor" }
renzora_theme = { path = "../../ui/renzora_theme" }
```

### 2. Implement EditorPanel

```rust
use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::PanelRegistry;
use renzora_theme::ThemeManager;

pub struct MyCustomPanel;

impl renzora_editor::EditorPanel for MyCustomPanel {
    fn id(&self) -> &str { "my_custom_panel" }
    fn title(&self) -> &str { "My Panel" }
    fn icon(&self) -> Option<&str> { Some(regular::CUBE) }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.resource::<ThemeManager>();
        ui.label("Hello from my extension!");
    }
}

pub struct MyExtensionPlugin;

impl Plugin for MyExtensionPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();
        let mut registry = world.remove_resource::<PanelRegistry>().unwrap_or_default();
        registry.register(MyCustomPanel);
        world.insert_resource(registry);
    }
}
```

### 3. Register the Plugin

The engine has two entry points:

- **`src/runtime.rs`** -- `build_runtime_app()` sets up the core engine (rendering, physics, audio, post-process effects). Plugins registered here run in both the editor and exported games.
- **`src/editor.rs`** -- `main()` calls `build_runtime_app()` then adds editor-only plugins inside `#[cfg(feature = "editor")]`. Plugins registered here only run in the editor.

Editor extensions (panels, inspectors, UI) go in `src/editor.rs`:

```rust
// src/editor.rs — inside the #[cfg(feature = "editor")] block
app.add_plugins(my_extension::MyExtensionPlugin);
```

Gameplay or rendering plugins that should also run in exported games go in `src/runtime.rs`:

```rust
// src/runtime.rs — inside build_runtime_app()
app.add_plugins(my_gameplay::GameplayPlugin);
```

### EditorPanel Trait Reference

```rust
pub trait EditorPanel: Send + Sync + 'static {
    fn id(&self) -> &str;                              // Unique ID
    fn title(&self) -> &str;                           // Display name
    fn icon(&self) -> Option<&str> { None }            // Phosphor icon
    fn ui(&self, ui: &mut egui::Ui, world: &World);   // Render content
    fn closable(&self) -> bool { true }                // Can user close it?
    fn min_size(&self) -> [f32; 2] { [100.0, 50.0] }  // Minimum dimensions
    fn default_location(&self) -> PanelLocation { ... } // Left|Right|Bottom|Center
}
```

## Creating Components

Components are data types that attach to entities. They show up in the inspector with editable fields. See `crates/editor/renzora_test_component/` for a complete example.

### 1. Create the Crate

```toml
# crates/editor/my_components/Cargo.toml
[package]
name = "my_components"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { version = "0.18" }
egui-phosphor = { version = "0.11", features = ["regular"] }
renzora_editor = { path = "../renzora_editor" }
```

### 2. Define the Component and Inspector

```rust
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{InspectorRegistry, InspectorEntry, FieldDef, FieldType, FieldValue};

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    pub has_shield: bool,
}

impl Default for Health {
    fn default() -> Self {
        Self { current: 100.0, max: 100.0, has_shield: false }
    }
}

fn health_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "health",
        display_name: "Health",
        icon: regular::HEART,
        category: "gameplay",
        has_fn: |world, entity| world.get::<Health>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Health::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Health>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Current",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 10_000.0 },
                get_fn: |world, entity| {
                    world.get::<Health>(entity).map(|h| FieldValue::Float(h.current))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.current = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 10_000.0 },
                get_fn: |world, entity| {
                    world.get::<Health>(entity).map(|h| FieldValue::Float(h.max))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.max = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Has Shield",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world.get::<Health>(entity).map(|h| FieldValue::Bool(h.has_shield))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.has_shield = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct MyComponentsPlugin;

impl Plugin for MyComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(health_entry());
        }
    }
}
```

### 3. Register the Plugin

Component plugins with editor inspectors go in `src/editor.rs` (editor-only). If the component also needs to exist at runtime (e.g., for scripting or gameplay), register it in both places or use a feature gate.

```rust
// src/editor.rs — inside the #[cfg(feature = "editor")] block
app.add_plugins(my_components::MyComponentsPlugin);
```

### Field Types

| FieldType | FieldValue | UI Widget |
|-----------|------------|-----------|
| `Float { speed, min, max }` | `Float(f32)` | Drag slider |
| `Vec3 { speed }` | `Vec3([f32; 3])` | XYZ drag fields |
| `Bool` | `Bool(bool)` | Checkbox |
| `Color` | `Color([f32; 3])` | Color picker |
| `String` | `String(String)` | Text input |
| `ReadOnly` | `ReadOnly(String)` | Non-editable label |

## Creating Post-Process Effects

Post-process effects are camera-attached components with WGSL shaders. See `crates/postprocessing/renzora_vignette/` for a complete example.

### 1. Create the Crate

```toml
# crates/postprocessing/my_effect/Cargo.toml
[package]
name = "my_effect"
version = "0.1.0"
edition = "2021"

[features]
default = []
editor = ["dep:renzora_editor", "dep:egui-phosphor"]

[dependencies]
bevy = { version = "0.18" }
serde = { version = "1", features = ["derive"] }
renzora_postprocess = { path = "../renzora_postprocess" }

# Editor-only deps
egui-phosphor = { version = "0.11", optional = true }
renzora_editor = { path = "../../editor/renzora_editor", optional = true }
```

### 2. Define the Effect

```rust
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use renzora_postprocess::PostProcessEffect;
use serde::{Serialize, Deserialize};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct MyEffectSettings {
    pub intensity: f32,
    pub enabled: f32,       // 1.0 = on, 0.0 = off (f32 for shader compatibility)
}

impl Default for MyEffectSettings {
    fn default() -> Self {
        Self { intensity: 0.5, enabled: 0.0 }
    }
}

impl PostProcessEffect for MyEffectSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/my_effect.wgsl".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}
```

### 3. Create the WGSL Shader

```wgsl
// assets/shaders/post_process/my_effect.wgsl

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct MyEffectSettings {
    intensity: f32,
    enabled: f32,
}
@group(0) @binding(2) var<uniform> settings: MyEffectSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if (settings.enabled < 0.5) {
        return color;
    }
    return mix(color, vec4(1.0, 0.0, 0.0, 1.0), settings.intensity * 0.1);
}
```

### 4. Create the Plugin

```rust
pub struct MyEffectPlugin;

impl Plugin for MyEffectPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MyEffectSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<MyEffectSettings>::default(),
        );

        #[cfg(feature = "editor")]
        {
            app.init_resource::<renzora_editor::InspectorRegistry>();
            let world = app.world_mut();
            if let Some(mut registry) = world.get_resource_mut::<renzora_editor::InspectorRegistry>() {
                registry.register(inspector_entry());
            }
        }
    }
}
```

### 5. Register the Plugin

Post-process effects run at runtime (they render in exported games), so register them in `src/runtime.rs`:

```rust
// src/runtime.rs — inside build_runtime_app()
app.add_plugins(my_effect::MyEffectPlugin);
```

```toml
# Root Cargo.toml — add to [dependencies]
my_effect = { path = "crates/postprocessing/my_effect" }

# Root Cargo.toml — add to [features] editor list (for inspector UI)
"my_effect/editor",
```

## Dynamic Plugins (DLL)

For plugins that load as dynamic libraries at runtime (hot-reloadable), use the `editor_plugin_api` crate. See `crates/plugins/websocket_plugin/` for a complete example.

```rust
use editor_plugin_api::prelude::*;

pub struct MyDynamicPlugin {
    status: String,
}

impl MyDynamicPlugin {
    pub fn new() -> Self {
        Self { status: "Ready".into() }
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.example.my-plugin", "My Plugin", "1.0.0")
            .author("Your Name")
            .description("Does something cool")
            .capability(PluginCapability::Panel)
    }
}

declare_plugin!(MyDynamicPlugin, MyDynamicPlugin::new());
```

### Plugin Capabilities

| Capability | Description |
|------------|-------------|
| `ScriptEngine` | Run scripts |
| `Gizmo` | Draw in the 3D viewport |
| `NodeType` | Custom node types |
| `Inspector` | Custom inspector widgets |
| `Panel` | Add editor panels |
| `MenuItem` | Add menu items |
| `AssetImporter` | Import custom asset formats |
| `Custom(String)` | User-defined capability |

## Exporting

Renzora uses `.rpak` files -- zstd-compressed archives containing all project assets. The export system supports multiple platforms and packaging modes.

### Export Templates

Export templates are pre-built runtime binaries for each target platform. When you export a game, the editor injects your project's assets into the template — so the template is a snapshot of the engine at build time.

**Building templates:**

```bash
# Desktop (current platform)
cargo dist-runtime

# Android (from Git Bash on Windows, or bash on Linux/macOS)
./scripts/build-android-template.sh              # ARM64 (phones, tablets, VR headsets)
./scripts/build-android-template.sh --x86_64     # ARM64 + x86_64 (emulator)
./scripts/build-android-template.sh --firetv     # Fire TV
```

Templates are installed automatically to:

```
Windows:  %APPDATA%\renzora\templates\
Linux:    ~/.config/renzora/templates/
macOS:    ~/Library/Application Support/renzora/templates/
```

Or use the "Install from file" button in the export overlay.

**When to rebuild templates:** Templates only contain the engine runtime (`src/runtime.rs`) — not your game assets. You need to rebuild templates when you:

- Add or remove plugins in `src/runtime.rs`
- Update Bevy or other engine dependencies
- Change any crate that the runtime depends on

Changes to `src/editor.rs` (editor-only code) do **not** require rebuilding templates. Changes to your game project (scenes, scripts, assets) also do not — those are packed into the `.rpak` at export time.

### Export Overlay

Open the export overlay from the editor. Options vary by platform:

- **Platform selection** -- Windows, Linux, macOS, Android, Fire TV, iOS, Web (with supported device list)
- **Packaging mode** (desktop only) -- Binary + `.rpak` (two files) or single executable (rpak appended)
- **Compression** -- zstd level 1-19
- **Window settings** (desktop only) -- Windowed, Fullscreen, or Borderless + resolution
- **Icon** -- Custom .png/.ico for the exported game
- **Output directory** -- Where the build goes

### How .rpak Works

An `.rpak` file contains all your project files (scenes, assets, scripts) compressed with zstd. At runtime:

1. The runtime checks for an embedded rpak inside itself (single binary mode)
2. Falls back to an adjacent `.rpak` file (e.g., `MyGame.rpak` next to `MyGame.exe`)
3. Falls back to `--project` CLI argument or local `project.toml`
4. On Android, reads `game.rpak` from the APK's `assets/` folder via the NDK AssetManager

### APK Signing

Android APKs are signed automatically during export using APK Signature Scheme v2 with ECDSA P-256. The editor generates a debug keypair and self-signed certificate on first use, stored in `%APPDATA%/renzora/signing/` (Windows) or `~/.config/renzora/signing/` (Linux/macOS). No Android SDK or external tools are required on the exporting machine.

### Supported Platforms

| Platform | Template | GPU Backend | Devices |
|----------|----------|-------------|---------|
| Windows (x64) | `renzora-runtime-windows-x64.exe` | Auto (DX12/Vulkan) | Desktop PCs, laptops, PCVR (SteamVR, Oculus Link) |
| Linux (x64) | `renzora-runtime-linux-x64` | Auto (Vulkan) | Desktop PCs, laptops, Steam Deck |
| macOS (x64) | `renzora-runtime-macos-x64` | Metal | Intel Macs |
| macOS (ARM64) | `renzora-runtime-macos-arm64` | Metal | Apple Silicon Macs (M1/M2/M3/M4) |
| Android (ARM64) | `renzora-runtime-android-arm64.apk` | Vulkan | Phones, tablets, Meta Quest, Pico, HTC Vive Focus |
| Android (x86_64) | `renzora-runtime-android-x86_64.apk` | Vulkan | Android emulators |
| Fire TV (ARM64) | `renzora-runtime-firetv-arm64.apk` | Vulkan | Fire TV Stick 4K Max, Fire TV Cube (3rd gen+) |
| Fire TV (ARM) | `renzora-runtime-firetv-arm.apk` | OpenGL ES | Fire TV Stick, Fire TV Stick 4K (1st gen), Fire TV Cube (1st/2nd gen) |
| iOS (ARM64) | `renzora-runtime-ios-arm64` | Metal | iPhone, iPad |
| Web (WASM) | `renzora-runtime-web-wasm32` | WebGPU/WebGL | All modern browsers |

#### Build Commands

| Template | Command |
|----------|---------|
| Desktop (current OS) | `cargo dist-runtime` |
| Android ARM64 | `./scripts/build-android-template.sh` |
| Android ARM64 + x86_64 | `./scripts/build-android-template.sh --x86_64` |
| Fire TV (ARM + ARM64) | `./scripts/build-android-template.sh --firetv` |

> **Note:** Fire TV (ARM) uses OpenGL ES instead of Vulkan because older Fire TV devices have PowerVR GPUs with broken Vulkan drivers. Fire TV (ARM64) targets newer devices with Mali GPUs that have full Vulkan support. The `--firetv` flag builds both templates automatically.

## Building Android Runtime

Build the Android runtime template APK so the editor can export games for Android devices. This is only needed if you're building template APKs yourself -- consumers just need the template file.

### Prerequisites

1. **Android Studio** (includes SDK, NDK, and Java JBR)
2. **cargo-ndk**: `cargo install cargo-ndk`
3. **Rust Android targets** (nightly):
   ```bash
   rustup target add aarch64-linux-android --toolchain nightly
   rustup target add x86_64-linux-android --toolchain nightly       # optional, for emulator
   rustup target add armv7-linux-androideabi --toolchain nightly     # optional, for older Fire TV
   ```

### Building the Template

A single script handles environment detection, Rust cross-compilation, native library bundling, and Gradle build:

```bash
./scripts/build-android-template.sh              # ARM64 template (Vulkan)
./scripts/build-android-template.sh --x86_64     # Also build x86_64 (for emulator)
./scripts/build-android-template.sh --firetv     # Fire TV: ARM (GLES) + ARM64 (Vulkan)
```

The script auto-detects `JAVA_HOME`, `ANDROID_HOME`, and `ANDROID_NDK_HOME` from standard install locations. Set them manually if needed.

The built template is installed to:
```
Windows:  %APPDATA%\renzora\templates\renzora-runtime-android-arm64.apk
Linux:    ~/.config/renzora/templates/renzora-runtime-android-arm64.apk
```

### CI/CD

Android templates are also built automatically via GitHub Actions on tagged releases and manual dispatch. The workflow cross-compiles on Ubuntu runners using `cargo-ndk` and uploads the unsigned template APK as a build artifact.

### Export Workflow

1. **Build the template** (or download from CI)
2. **Export from the editor** -- select Android in the export overlay, which injects your game's `.rpak` into the template APK and signs it automatically
3. **Install on device** -- the exported APK is ready to install, no additional signing step needed

### How It Works

- `cargo ndk` cross-compiles the `renzora_android` crate to `libmain.so` for each target architecture
- `libc++_shared.so` is copied from the NDK sysroot alongside the native library
- Gradle packages everything into an APK using `GameActivity` (from `androidx.games:games-activity:2.0.2`)
- At export time, the editor injects `game.rpak` into the APK's `assets/` folder and signs it with APK Signature Scheme v2 (pure Rust, no SDK needed)
- At runtime, the VFS reads `game.rpak` from APK assets via the Android AssetManager NDK API

### Troubleshooting Android

| Issue | Fix |
|-------|-----|
| `ClassNotFoundException: GameActivity` | Ensure `games-activity` version matches `android-activity` Rust crate (currently 2.0.2) |
| `UnsatisfiedLinkError: libc++_shared.so` | The build script copies this automatically; if building manually, copy from NDK sysroot |
| Blank screen (app runs but no content) | Ensure the rpak was injected into `assets/game.rpak` in the APK |
| `SDK location not found` | Create `android/local.properties` with `sdk.dir=/path/to/Android/Sdk` |

## Cargo Features

| Feature | Description |
|---------|-------------|
| `editor` | Full editor with UI, asset browser, scene editing (default) |
| `solari` | Raytraced GI, DLSS, and meshlet virtual geometry (requires Vulkan SDK + DLSS SDK) |
| `dynamic` | Dynamic linking for faster dev builds |

## Supported File Formats

| Format | Type |
|--------|------|
| `.glb` / `.gltf` | 3D models (meshes, materials, animations, skeletons) |
| `.obj` | 3D models (meshes) |
| `.fbx` | 3D models (meshes, skeletons) |
| `.ron` | Scene files (Bevy DynamicScene) |
| `.rpak` | Compressed asset archives (exported games) |
| `.rhai` | Script files |
| `.blueprint` | Visual script graphs (compile to Rhai) |
| `.material_bp` | Material blueprint graphs (compile to WGSL) |
| `.particle` | Particle effect definitions |
| `.png` / `.jpg` / `.jpeg` | Textures |
| `.hdr` / `.exr` | HDR environment maps |
| `.ogg` / `.mp3` / `.wav` / `.flac` | Audio files (Kira 0.12) |

## Testing

```bash
cargo test                                  # full test suite
cargo test -- blueprint::graph_tests        # specific module
cargo test -- scripting::tests
cargo test -- component_system::tests
cargo test -- docking
cargo test -- keybindings
```

## Troubleshooting

### Runtime crashes immediately

Run from a terminal to see error output:
```bash
& "./MyGame.exe" --project "./my_project"
```

### Small runtime binary (~1.5MB)

Bevy was compiled with dynamic linking. Use the dist build:
```bash
cargo dist-runtime
```

### Release build fails with "Application Control policy has blocked this file"

Windows Smart App Control is blocking build script executables. Open **Windows Security > App & Browser Control > Smart App Control** and turn it off.

### Export shows "Template not installed"

Build the runtime template and install it:
```bash
cargo dist-runtime
```
Then copy the binary to the templates directory, or use the "Install from file" button in the export overlay.

## License

Dual-licensed under MIT or Apache 2.0, at your option.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)
