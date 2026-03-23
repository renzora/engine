# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/).

![Renzora Editor](assets/previews/interface.png)

> **Warning:** This engine is in early alpha. You will encounter bugs, incomplete features, and unexpected behavior. APIs and file formats may change without notice between versions.

> **AI-Assisted Development:** This project is built with the assistance of AI code generation tools, including Claude by Anthropic. AI is used throughout the development process for writing code, designing systems, generating documentation, and solving engineering problems. This is a deliberate choice — AI tooling allows a small team to move fast and build an ambitious engine that would otherwise require significantly more time and resources. If the use of AI-generated code is a concern for you, this project may not be the right fit. There are many excellent open-source game engines and frameworks built entirely by hand — [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), and [Fyrox](https://fyrox.rs/) are all great alternatives worth exploring.

## Table of Contents

1. [Documentation](#documentation)
2. [Prerequisites](#prerequisites)
3. [Building & Running](#building--running)
4. [Building for WASM](#building-for-wasm)
5. [Mesh Optimization](#mesh-optimization)
6. [Creating Extensions](#creating-extensions)
7. [Creating Scripting Extensions](#creating-scripting-extensions)
8. [Creating Components](#creating-components)
9. [Creating Post-Process Effects](#creating-post-process-effects)
10. [Dynamic Plugins (DLL)](#dynamic-plugins-dll)
11. [Exporting](#exporting)
12. [Building Android Runtime](#building-android-runtime)
13. [Building iOS / tvOS Runtime](#building-ios--tvos-runtime)
14. [Cargo Features](#cargo-features)
15. [Supported File Formats](#supported-file-formats)
16. [Testing](#testing)
17. [Troubleshooting](#troubleshooting)
18. [License](#license)

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
cargo server                 # build + run the dedicated server
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
cargo release-server         # optimized dedicated server
```

### Dist Builds (max optimization, fat LTO, stripped)

```bash
cargo dist-runtime           # runtime only
cargo dist-server            # server only
cargo dist-editor            # editor only
cargo dist-all               # editor + runtime + server in one pass
```

The `dist` profile uses fat LTO, single codegen unit, `opt-level = "z"`, panic = abort, and symbol stripping for minimum binary size.

### Dist with UPX Compression

Requires [cargo-make](https://github.com/sagiegurari/cargo-make) and [UPX](https://upx.github.io/):

```bash
cargo install cargo-make

# UPX:
# Windows:  winget install upx
# Linux:    sudo apt install upx
# macOS:    brew install upx
```

```bash
cargo make dist              # build all 3 + compress with UPX
cargo make dist-build        # build all 3 without compression
cargo make dist-runtime      # build just runtime
cargo make dist-server       # build just server
cargo make dist-editor       # build just editor
```

UPX typically achieves ~70-75% size reduction on the engine binaries.

### Running the Runtime with a Project

```bash
# PowerShell
& "./target/debug/renzora-runtime.exe" --project "C:\Users\you\Documents\my_game"

# Bash / Linux / macOS
./target/debug/renzora-runtime --project ~/Documents/my_game
```

The runtime also looks for `project.toml` in the current directory if `--project` is not specified.

### Architecture

The engine has three binaries that share a common runtime library:

- **`src/editor.rs`** -- Editor binary. Calls `build_runtime_app()` then adds editor plugins (UI, viewport, inspector, etc.) behind `#[cfg(feature = "editor")]`.
- **`src/runtime.rs`** -- Shared runtime setup. Registers plugins, post-process effects, environment plugins. With the `server` feature, uses `DefaultPlugins` in headless mode (no window, no audio, no postprocessing).
- **`src/server.rs`** -- Dedicated server binary. Calls `build_runtime_app()` with `--features server` (headless) then adds `NetworkServerPlugin`.

All three binaries use the same runtime core. The `editor` feature controls whether the editor UI is included, and the `server` feature controls headless mode (no rendering/audio/postprocessing).

## Building for WASM

The engine compiles to WebAssembly for browser deployment. Both the editor and the game runtime can run in the browser. Some subsystems are automatically disabled on WASM (audio, networking, Lua scripting) while the rest works natively via WebGPU.

### Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
cargo install wasm-opt       # or: pip install wasm-opt
cargo install brotli         # pre-compression for serving
```

> **Note:** `cargo make dist-web` and `cargo make dist-web-runtime` will automatically install missing tools (`wasm-bindgen`, `wasm-opt`, `brotli`) before building.

### WASM Builds

There are two WASM targets — the editor (for running in the browser) and the runtime (export template for shipping games):

| What | Command | Output |
|------|---------|--------|
| Editor (compile only) | `cargo dist-web-editor` | `app/wasm32-unknown-unknown/dist/renzora.wasm` |
| Editor (full pipeline) | `cargo make dist-web` | `web/` (serve with Vite) |
| Runtime template (full pipeline) | `cargo make dist-web-runtime` | `target/dist/renzora-runtime-web-wasm32.zip` |

### Editor in Browser

The `web/` directory contains a Vite project for running the editor in the browser:

```bash
cargo make dist-web           # compile editor + wasm-bindgen + wasm-opt + brotli → web/
cd web
npm install                   # first time only
npm run dev                   # start Vite dev server on localhost:3000
```

For production:

```bash
cd web
npm run build                 # Vite production bundle → web/dist/
npm run preview               # preview the production build locally
```

If the wasm is already built, `npm run dev` skips recompilation and just starts Vite.

### Game Export Template

The runtime wasm template is a `.zip` containing `renzora-runtime.js` + `renzora-runtime_bg.wasm`. It works like every other export template — build it once, install it, then export games from the editor overlay:

```bash
cargo make dist-web-runtime   # compile + bind + optimize + zip → target/dist/renzora-runtime-web-wasm32.zip
```

Install the template by copying it to the templates directory or using the "Install from file" button in the export overlay:

```
Windows:  %APPDATA%\renzora\templates\renzora-runtime-web-wasm32.zip
Linux:    ~/.config/renzora/templates/renzora-runtime-web-wasm32.zip
macOS:    ~/Library/Application Support/renzora/templates/renzora-runtime-web-wasm32.zip
```

When you select **Web (WASM)** in the export overlay, the editor:
1. Packs your project assets into an `.rpak`
2. Extracts the template zip (JS glue + wasm binary)
3. Generates an `index.html` with your project name
4. Bundles everything into `{project}-web.zip`

Unzip to any static host and it runs.

### Deploying

- **GitHub Pages** — push the unzipped contents; brotli compression is automatic (~7MB over the wire)
- **Netlify / Vercel / Cloudflare Pages** — same, just point to the output directory
- **Any HTTP server** — files must be served over HTTP, not opened as local files

### WASM Limitations

| Subsystem | Status |
|-----------|--------|
| Rendering (WebGPU) | Works (Chrome 113+, Edge 113+, Firefox Nightly) |
| Physics (Avian) | Works |
| Particles (Hanabi) | Works (requires WebGPU compute shaders) |
| Scripting (Rhai) | Works |
| Visual scripting (Blueprints) | Works |
| Animation | Works |
| Game UI | Works |
| Post-processing effects | Works |
| Terrain | Works |
| Screen-space RT effects | Disabled (storage texture limits) |
| Audio (Kira) | Disabled (no cpal backend for WASM) |
| Networking (Lightyear) | Disabled (no UDP sockets on WASM) |
| Scripting (Lua) | Disabled (requires C compiler) |
| File dialogs (rfd) | Disabled |
| Clipboard (arboard) | Disabled |

### Compression

The `cargo make dist-web` pipeline automatically runs `wasm-opt -Oz` and `brotli -q 11`, producing both `.wasm` and `.wasm.br` in `web/`:

| Stage | Typical Size |
|-------|-------------|
| Raw (pre-opt) | ~48 MB |
| After `wasm-opt -Oz` | ~37 MB |
| After Brotli (transfer size) | ~8.5 MB |

Most web servers (nginx, Cloudflare, Vercel, Netlify) serve the `.br` file automatically when the browser sends `Accept-Encoding: br`.

## Mesh Optimization

The engine includes optional meshoptimizer-based mesh processing at both import and export time, powered by the `meshopt` crate.

### Import Optimization (lossless)

When importing 3D models through the import overlay, three lossless optimizations are available (all enabled by default):

| Option | Effect |
|--------|--------|
| Optimize vertex cache | Reorders triangles for GPU vertex cache locality |
| Optimize overdraw | Reorders triangles to reduce pixel overdraw |
| Optimize vertex fetch | Reorders vertices for vertex fetch cache efficiency |

These don't change the visual output -- they just make meshes render faster. The import runs on a background thread with per-file progress reporting.

### Export Optimization (lossy, optional)

When exporting a project, additional mesh processing options are available in the export overlay under "Mesh Optimization":

| Option | Effect |
|--------|--------|
| Simplify meshes | Reduces triangle count by a configurable ratio (0.1 - 1.0) |
| Quantize vertex attributes | Reduces vertex data precision for smaller files |
| Generate LODs | Creates N levels of detail (1-5) as separate `.glb` entries |

The export pipeline only packs assets that are actually referenced from scene files (BFS from `project.toml` -> main scene -> asset references). No unused assets are included.

### Export Pipeline Order

1. Scan project entry points and pack only referenced assets
2. Strip editor-only components from scene files
3. Optimize meshes (vertex cache, overdraw, vertex fetch + optional simplify/quantize)
4. Generate LOD variants (if enabled)
5. Compress with zstd and write `.rpak`
6. Copy/append to runtime template binary

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
bevy = { version = "0.18", default-features = false }
bevy_egui = "0.39"
egui-phosphor = { version = "0.11", features = ["regular"] }
renzora_editor = { path = "../renzora_editor" }
renzora_theme = { path = "../../ui/renzora_theme" }
```

> **Important:** Always use `default-features = false` on the `bevy` dependency in sub-crates. The root `Cargo.toml` controls which Bevy features are active. Without this, sub-crates pull in Bevy's full default features (including `file_watcher`) which breaks WASM builds via Cargo feature unification.

### 2. Implement EditorPanel

```rust
use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::AppEditorExt;
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
        app.register_panel(MyCustomPanel);
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

## Creating Scripting Extensions

Scripting extensions let any crate register custom script functions, inject per-entity data, and process custom commands — without modifying the scripting crate. See `crates/core/renzora_gauges/` for a complete example.

### 1. Create the Crate

```toml
# crates/core/my_system/Cargo.toml
[package]
name = "my_system"
version = "0.1.0"
edition = "2021"

[features]
default = []
lua = ["dep:mlua"]
rhai = ["dep:rhai"]

[dependencies]
bevy = { version = "0.18", default-features = false }
renzora_scripting = { path = "../renzora_scripting" }
rhai = { version = "1.21", features = ["sync"], optional = true }

# Lua is native-only (requires C compiler, cannot target WASM)
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
mlua = { version = "0.10", features = ["lua54", "vendored", "send"], optional = true }
```

Enable the lua/rhai features in the root `Cargo.toml`:

```toml
[features]
editor = [
    "my_system/lua",
    "my_system/rhai",
]
```

### 2. Define Extension Data and Commands

Extension data is typed data injected into the script context before execution. Commands are actions scripts can trigger.

```rust
use renzora_scripting::extension::{ExtensionData, ScriptExtension};
use renzora_scripting::macros::push_ext_command;
use std::collections::HashMap;

/// Data injected into scripts each frame.
#[derive(Clone, Default)]
pub struct MyData {
    pub values: HashMap<String, f32>,
}

/// Commands scripts can issue. The macro auto-implements ScriptExtensionCommand.
renzora_scripting::script_extension_command! {
    #[derive(Debug)]
    pub enum MyCommand {
        SetValue { key: String, value: f32 },
        DoAction { target: u64 },
    }
}
```

### 3. Register Script Functions

Use `dual_register!` to define functions once — both Lua and Rhai bindings are generated automatically. The body uses standard Rust types; type conversion (e.g. Rhai `ImmutableString` -> `String`) is handled by the macro.

Supported parameter types: `String`, `f64`, `i64`, `bool`.

```rust
renzora_scripting::dual_register! {
    lua_fn = register_my_lua,
    rhai_fn = register_my_rhai,

    fn my_set(key: String, value: f64) {
        push_ext_command(MyCommand::SetValue { key, value: value as f32 });
    }

    fn my_action(target: i64) {
        push_ext_command(MyCommand::DoAction { target: target as u64 });
    }
}
```

For functions that need language-specific features (varargs, reading from context tables, returning values), implement them manually per-backend.

### 4. Implement ScriptExtension

```rust
pub struct MyScriptExtension;

impl ScriptExtension for MyScriptExtension {
    fn name(&self) -> &str { "MySystem" }

    fn populate_context(
        &self,
        world: &bevy::prelude::World,
        entity: bevy::prelude::Entity,
        data: &mut ExtensionData,
    ) {
        let mut my_data = MyData::default();
        // ... populate from components/resources ...
        data.insert(my_data);
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        register_my_lua(lua);
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        let Some(my_data) = data.get::<MyData>() else { return };
        renzora_scripting::macros::lua_set_map(lua, "_my_data", &my_data.values);
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        register_my_rhai(engine);
    }

    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        let Some(my_data) = data.get::<MyData>() else { return };
        renzora_scripting::macros::rhai_set_map(scope, "_my_data", &my_data.values);
    }
}
```

### 5. Process Commands

```rust
use bevy::prelude::*;
use renzora_scripting::systems::execution::ScriptCommandQueue;
use renzora_scripting::ScriptCommand;

fn process_my_commands(cmd_queue: Res<ScriptCommandQueue>) {
    for (_source_entity, cmd) in &cmd_queue.commands {
        let ScriptCommand::Extension(ext_cmd) = cmd else { continue };
        let Some(my_cmd) = ext_cmd.as_any().downcast_ref::<MyCommand>() else { continue };

        match my_cmd {
            MyCommand::SetValue { key, value } => {
                // Handle the command
            }
            MyCommand::DoAction { target } => {
                // Handle the command
            }
        }
    }
}
```

### 6. Register the Plugin

```rust
use renzora_scripting::{ScriptExtensions, ScriptingSet};

pub struct MySystemPlugin;

impl Plugin for MySystemPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .resource_mut::<ScriptExtensions>()
            .register(MyScriptExtension);

        app.add_systems(
            Update,
            process_my_commands.in_set(ScriptingSet::CommandProcessing),
        );
    }
}
```

### Extension API Reference

| Macro / Helper | Purpose |
|---|---|
| `script_extension_command!` | Define command enum + auto-impl `ScriptExtensionCommand` |
| `dual_register!` | Define functions once, generate Lua + Rhai bindings |
| `push_ext_command(cmd)` | Push extension command (shorthand) |
| `lua_set_map(lua, name, &HashMap)` | Set Lua global table from `HashMap<String, f32>` |
| `lua_set_nested_map(lua, name, &HashMap)` | Set nested Lua table from `HashMap<K, HashMap<String, f32>>` |
| `rhai_set_map(scope, name, &HashMap)` | Push Rhai scope map from `HashMap<String, f32>` |
| `rhai_set_nested_map(scope, name, &HashMap)` | Push nested Rhai scope map |

### How It Works

1. **Plugin build** — your plugin registers a `ScriptExtension` on the `ScriptExtensions` resource
2. **Each frame** — the execution system calls `populate_context()` for each script entity, passing `&World` and `&mut ExtensionData`
3. **Script execution** — backends call `register_*_functions()` (once) and `setup_*_context()` (per entity) so your functions and data are available in scripts
4. **Command processing** — scripts call your functions which push commands via `push_ext_command()`
5. **After execution** — your system in `ScriptingSet::CommandProcessing` reads commands from `ScriptCommandQueue`, downcasts to your command type, and processes them

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
bevy = { version = "0.18", default-features = false }
egui-phosphor = { version = "0.11", features = ["regular"] }
renzora_editor = { path = "../renzora_editor" }
```

### 2. Define the Component

Use `#[derive(Inspectable)]` to auto-generate all inspector boilerplate:

```rust
use bevy::prelude::*;
use renzora_editor::{AppEditorExt, Inspectable};

#[derive(Component, Default, Inspectable)]
#[inspectable(name = "Health", icon = "HEART", category = "gameplay")]
pub struct Health {
    #[field(speed = 1.0, min = 0.0, max = 10000.0)]
    pub current: f32,
    #[field(speed = 1.0, min = 1.0, max = 10000.0)]
    pub max: f32,
    #[field(name = "Shield")]
    pub has_shield: bool,
}

pub struct MyComponentsPlugin;

impl Plugin for MyComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<Health>();
    }
}
```

The `Inspectable` derive generates the `InspectableComponent` trait impl automatically. Field types are inferred from Rust types (`f32` -> drag slider, `bool` -> checkbox, `String` -> text input, `Vec3` -> XYZ fields).

#### `#[field(...)]` Attributes

| Attribute | Description |
|-----------|-------------|
| `speed = 0.1` | Drag speed for float/vec3 fields |
| `min = 0.0` | Minimum value for float fields |
| `max = 100.0` | Maximum value for float fields |
| `name = "Display Name"` | Override the field's display name (default: title-cased) |
| `skip` | Hide the field from the inspector |
| `readonly` | Show as non-editable label |

#### `#[inspectable(...)]` Attributes

| Attribute | Description |
|-----------|-------------|
| `name = "Health"` | Display name in inspector (default: struct name) |
| `icon = "HEART"` | Phosphor icon name (default: `CUBE`) |
| `category = "gameplay"` | Inspector category for grouping |
| `type_id = "health"` | Unique ID (default: snake_case struct name) |

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
bevy = { version = "0.18", default-features = false }
serde = { version = "1", features = ["derive"] }
renzora_postprocess = { path = "../renzora_postprocess" }

# Editor-only deps
egui-phosphor = { version = "0.11", optional = true }
renzora_editor = { path = "../../editor/renzora_editor", optional = true }
```

### 2. Define the Effect

Use `#[post_process]` to generate all derives, padding, `Default`, `PostProcessEffect`, and inspector impl:

```rust
use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "my_effect.wgsl", name = "My Effect", icon = "SPARKLE")]
pub struct MyEffectSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub intensity: f32,
}
```

The `#[post_process]` macro automatically:
- Adds all required derives (`Component`, `Clone`, `Copy`, `Reflect`, `Serialize`, `Deserialize`, `ShaderType`, `ExtractComponent`)
- Adds an `enabled: f32` field (1.0 = on, 0.0 = off)
- Adds padding fields for 16-byte GPU alignment
- Generates `Default` impl (respects `#[field(default = ...)]`)
- Generates `PostProcessEffect` impl (shader path)
- Generates `InspectableComponent` impl (behind `#[cfg(feature = "editor")]`)

### 3. Create the WGSL Shader

The WGSL struct must match the Rust struct layout (user fields + padding + enabled):

```wgsl
// src/my_effect.wgsl
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct MyEffectSettings {
    intensity: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
    _padding6: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: MyEffectSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }
    return mix(color, vec4(1.0, 0.0, 0.0, 1.0), settings.intensity * 0.1);
}
```

> **Note:** The macro pads to a minimum of 8 f32s (2 vec4s). For a struct with 1 user field + enabled = 2 fields, you get 6 padding fields. The formula is: `padding = max(8, next_multiple_of_4(fields + 1)) - fields - 1`.

### 4. Create the Plugin

```rust
pub struct MyEffectPlugin;

impl Plugin for MyEffectPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "my_effect.wgsl");
        app.register_type::<MyEffectSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<MyEffectSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<MyEffectSettings>();
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

### How Export Works

The export pipeline only packs files that are actually referenced from your project:

1. **Scan** -- BFS from `project.toml` -> main scene -> all quoted asset paths in scene/script/config files
2. **Pack** -- Only referenced files are read from disk and added to the archive
3. **Strip** -- Editor-only components (e.g. `OrbitCameraState`) are removed from scene RON files
4. **Optimize** -- Meshopt runs on `.glb` files (vertex cache, overdraw, vertex fetch, optional simplify/LOD)
5. **Compress** -- zstd compression at configurable level (1-19)
6. **Write** -- Output as standalone `.rpak` or appended to a runtime template binary

For server exports, visual components (`MeshInstanceData`, `MeshColor`, etc.) and rendering assets are additionally stripped.

### Export Templates

Export templates are pre-built runtime binaries for each target platform. When you export a game, the editor injects your project's assets into the template — so the template is a snapshot of the engine at build time.

**Building templates:**

```bash
# Desktop (current platform)
cargo make dist-runtime           # game runtime template
cargo make dist-server            # dedicated server template

# Android
cargo make dist-android-arm64     # ARM64 (phones, tablets, VR headsets)
cargo make dist-android-x86       # x86_64 (emulator)
cargo make dist-android-firetv    # Fire TV ARM64
cargo make dist-android-all       # All Android templates
```

All templates are output to `target/templates/`:

```
target/templates/
  renzora-runtime-windows-x64.exe          # Desktop
  renzora-runtime-android-arm64.apk        # Android ARM64
  renzora-runtime-android-x86_64.apk       # Android x86_64
  renzora-runtime-firetv-arm64.apk         # Fire TV
  renzora-runtime-web-wasm32.zip           # Web
```

Android templates are also installed to the editor's template cache:

```
Windows:  %APPDATA%\renzora\templates\
Linux:    ~/.config/renzora/templates/
macOS:    ~/Library/Application Support/renzora/templates/
```

Or use the "Install from file" button in the export overlay.

**When to rebuild templates:** Templates only contain the engine runtime (`src/runtime.rs`) and server (`src/server.rs`) — not your game assets. You need to rebuild templates when you:

- Add or remove plugins in `src/runtime.rs`
- Update Bevy or other engine dependencies
- Change any crate that the runtime depends on

Changes to `src/editor.rs` (editor-only code) do **not** require rebuilding templates. Changes to your game project (scenes, scripts, assets) also do not — those are packed into the `.rpak` at export time.

### Export Overlay

Open the export overlay from the editor. Options vary by platform:

- **Platform selection** -- Windows, Linux, macOS, Android, Fire TV, iOS, Web (with supported device list)
- **Packaging mode** (desktop only) -- Binary + `.rpak` (two files) or single executable (rpak appended)
- **Compression** -- zstd level 1-19
- **Mesh optimization** -- Simplify meshes (with ratio slider), quantize vertex attributes, generate LODs (with level count)
- **Window settings** (desktop only) -- Windowed, Fullscreen, or Borderless + resolution
- **Include dedicated server** (desktop only) -- Exports a server binary alongside the game with stripped assets (scenes, scripts, config only — no textures, audio, models, or shaders)
- **Icon** -- Custom .png/.ico for the exported game
- **Output directory** -- Where the build goes

The export runs on a background thread with real-time progress reporting (per-file packing, mesh optimization, LOD generation, writing).

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
| Fire TV | `renzora-runtime-firetv-arm64.apk` | Vulkan | Fire TV Stick 4K Max, Fire TV Cube (3rd gen+) |
| iOS (ARM64) | `renzora-runtime-ios-arm64` | Metal | iPhone, iPad |
| Web (WASM) | `renzora-runtime-web-wasm32` | WebGPU/WebGL | All modern browsers |

#### Build Commands

| Template | Command |
|----------|---------|
| Desktop (current OS) | `cargo make dist-runtime` |
| Android ARM64 | `cargo make dist-android-arm64` |
| Android x86_64 | `cargo make dist-android-x86` |
| Fire TV | `cargo make dist-android-firetv` |
| All Android | `cargo make dist-android-all` |
| Web | `cargo make dist-web-runtime` |

#### Unsupported Devices

Older Fire TV devices (Stick 1st/2nd gen, Stick 4K 1st gen, Cube 1st/2nd gen) are **not supported**. These use 32-bit ARM SoCs with PowerVR GPUs that lack the minimum GPU feature set required by wgpu/Bevy (texture view dimensions, downlevel limits). Only ARM64 Fire TV devices with Vulkan-capable Mali GPUs are supported.

## Building Android Runtime

Build the Android runtime template APK so the editor can export games for Android devices. This is only needed if you're building template APKs yourself -- consumers just need the template file.

### Prerequisites

1. **Android Studio** (includes SDK, NDK, and Java JBR)
2. **cargo-ndk**: `cargo install cargo-ndk`
3. **Rust Android targets** (nightly):
   ```bash
   rustup target add aarch64-linux-android --toolchain nightly
   rustup target add x86_64-linux-android --toolchain nightly       # optional, for emulator
   ```

### Building the Template

Use `cargo make` to build (handles cross-platform script selection automatically):

```bash
cargo make dist-android-arm64     # Android ARM64 (Vulkan)
cargo make dist-android-x86       # Android x86_64 (Vulkan)
cargo make dist-android-firetv    # Fire TV ARM64 (Vulkan)
cargo make dist-android-all       # All templates
```

The underlying scripts auto-detect `JAVA_HOME`, `ANDROID_HOME`, and `ANDROID_NDK_HOME` from standard install locations. Set them manually if needed.

Templates are output to `target/templates/` and also installed to the editor's template cache:
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

## Building iOS / tvOS Runtime

Build the iOS or tvOS runtime template so the editor can export games for iPhone, iPad, and Apple TV. This requires macOS with Xcode.

### Prerequisites

1. **Xcode** with iOS and/or tvOS SDKs (`xcode-select --install`)
2. **Rust Apple targets** (nightly):
   ```bash
   rustup target add aarch64-apple-ios --toolchain nightly          # iOS device
   rustup target add aarch64-apple-ios-sim --toolchain nightly      # iOS simulator
   rustup target add aarch64-apple-tvos --toolchain nightly         # Apple TV
   rustup target add aarch64-apple-tvos-sim --toolchain nightly     # Apple TV simulator
   ```

### Building the Template

```bash
cargo make dist-ios              # iOS device (ARM64)
cargo make dist-ios-sim          # iOS simulator (ARM64)
cargo make dist-tvos             # Apple TV (ARM64)
cargo make dist-tvos-sim         # Apple TV simulator (ARM64)
```

Or use the build script directly:

```bash
./ios/build-template.sh                    # iOS device
./ios/build-template.sh --simulator        # iOS simulator
./ios/build-template.sh --tvos             # Apple TV
./ios/build-template.sh --tvos-simulator   # Apple TV simulator
```

Templates are output to `target/templates/` and installed to the editor's template cache:
```
macOS:  ~/Library/Application Support/renzora/templates/renzora-runtime-ios-arm64.zip
```

### Cargo Aliases

```bash
cargo build-ios          # Build iOS static lib (release)
cargo build-ios-sim      # Build for iOS simulator
cargo build-tvos         # Build Apple TV static lib (release)
cargo build-tvos-sim     # Build for Apple TV simulator
cargo dist-ios           # Build with max optimization (dist profile)
cargo dist-tvos          # Build with max optimization (dist profile)
```

### Export Workflow

1. **Build the template** on macOS (or download from CI)
2. **Export from the editor** -- select iOS or Apple TV in the export overlay, which injects your game's `.rpak` into the app bundle and outputs an `.ipa`
3. **Sign and distribute** -- use Xcode or `codesign` to sign for TestFlight / App Store / ad-hoc distribution

### How It Works

- `cargo build` cross-compiles the `renzora_ios` crate to a static library (`librenzora_ios.a`) for the target architecture
- `xcodebuild` links the static library with a Swift `AppDelegate` that calls the Rust `renzora_main()` entry point
- The Xcode project links Metal, GameController, AudioToolbox, AVFoundation, and other system frameworks
- At export time, the editor injects `game.rpak` into the `.app` bundle and packages it as an `.ipa`
- At runtime, the VFS reads `game.rpak` from the app bundle via CoreFoundation's `CFBundleCopyResourceURL`
- iOS covers both iPhone and iPad (universal binary with `TARGETED_DEVICE_FAMILY = "1,2"`)
- tvOS uses a separate Info.plist tuned for Apple TV (landscape, focus-based navigation, no touch)

### Supported Devices

| Template | Devices |
|----------|---------|
| iOS (ARM64) | iPhone, iPad |
| Apple TV (ARM64) | Apple TV 4K, Apple TV HD |

### Troubleshooting iOS / tvOS

| Issue | Fix |
|-------|-----|
| `error: Rust target not installed` | Run `rustup target add aarch64-apple-ios` (or the target you need) |
| `xcodebuild` fails with signing error | Templates are built unsigned (`CODE_SIGNING_ALLOWED=NO`); signing happens at distribution time |
| Blank screen (app runs but no content) | Ensure the rpak was injected into the `.app` bundle as `game.rpak` |
| Scripts not running on device | Verify scripts are packed into the rpak (check export log for `.lua`/`.rhai` files) |
| `Library not found for -lrenzora_ios` | The build script copies the `.a` to `ios/libs/`; ensure `LIBRARY_SEARCH_PATHS` points there |

## Cargo Features

| Feature | Description | Default |
|---------|-------------|---------|
| `editor` | Full editor with UI, asset browser, scene editing | Yes |
| `dynamic` | Dynamic linking for faster dev builds | Yes |
| `native` | Native platform support (file watcher, gamepad, x11/wayland) | Yes |
| `server` | Headless mode for dedicated server (no window, no rendering, no audio) | No |
| `solari` | Raytraced GI, DLSS, and meshlet virtual geometry (requires Vulkan SDK + DLSS SDK) | No |

### Bevy Feature Management

All sub-crates use `bevy = { version = "0.18", default-features = false }`. The root `Cargo.toml` is the single source of truth for which Bevy features are active. This prevents Cargo feature unification from accidentally enabling features like `file_watcher` that break WASM builds.

The `native` feature enables `bevy/default_platform` which includes file watching, gamepad support, and platform-specific window backends. It's included in `default` but excluded for WASM builds (`--no-default-features`).

### WASM-Incompatible Subsystems

These subsystems are automatically gated behind `cfg(not(target_arch = "wasm32"))`:

| Crate | Reason |
|-------|--------|
| `renzora_audio` (Kira) | cpal backend requires native audio API |
| `renzora_network` (Lightyear) | UDP sockets not available in browsers |
| `renzora_scripting` (Lua only) | mlua requires C compiler for vendored Lua |
| `renzora_rpak` (packer only) | Filesystem packing is editor/build-time only |

Rhai scripting works on WASM (pure Rust). The rpak *reader* works on WASM.

## Supported File Formats

| Format | Type |
|--------|------|
| `.glb` / `.gltf` | 3D models (meshes, materials, animations, skeletons) |
| `.obj` | 3D models (meshes) |
| `.fbx` | 3D models (meshes, skeletons) |
| `.stl` | 3D models (meshes) |
| `.ply` | 3D models (point clouds, meshes) |
| `.ron` | Scene files (Bevy DynamicScene) |
| `.rpak` | Compressed asset archives (exported games) |
| `.rhai` | Script files |
| `.lua` | Script files (native builds only) |
| `.blueprint` | Visual script graphs |
| `.material` | Material graphs (compile to WGSL) |
| `.particle` | Particle effect definitions |
| `.png` / `.jpg` / `.jpeg` | Textures |
| `.hdr` / `.exr` | HDR environment maps |
| `.ogg` / `.mp3` / `.wav` / `.flac` | Audio files (Kira 0.12, native only) |

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

Build the runtime and/or server template:
```bash
cargo make dist-runtime      # game runtime template → target/templates/
cargo make dist-server       # dedicated server template
```
Or use the "Install from file" button in the export overlay.

### WASM build fails with "file_watcher" error

A sub-crate is pulling in Bevy with default features. Ensure all sub-crate `Cargo.toml` files use:
```toml
bevy = { version = "0.18", default-features = false }
```

### WASM build fails with "getrandom" error

Add both getrandom versions with JS/WASM features to the root `Cargo.toml`:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom_02 = { package = "getrandom", version = "0.2", features = ["js"] }
getrandom_03 = { package = "getrandom", version = "0.3", features = ["wasm_js"] }
```

## License

Dual-licensed under MIT or Apache 2.0, at your option.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)
