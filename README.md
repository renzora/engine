# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/). Currently in **alpha** — actively developing toward feature parity with Bevy's full capabilities.

![Renzora Editor](assets/previews/interface.png)

> **Warning:** This engine is in early alpha. You will encounter bugs, incomplete features, and unexpected behavior. APIs and file formats may change without notice between versions.

## Table of Contents

1. [Project Status](#project-status)
2. [Documentation](#documentation)
3. [Prerequisites](#prerequisites)
4. [Building](#building)
5. [Testing](#testing)
6. [Cargo Features](#cargo-features)
7. [Supported File Formats](#supported-file-formats)
8. [Troubleshooting](#troubleshooting)
9. [License](#license)

## Project Status

**Alpha** — Core systems are functional and the editor is usable for scene composition, scripting, and game export. Not yet recommended for production use.

## Documentation

| Guide | Description |
|-------|-------------|
| **[Engine User Guide](docs/ENGINE_USER_GUIDE.md)** | Viewport controls, keyboard shortcuts, transform gizmos, terrain editing, scene management, components, and more |
| **[Physics Guide](docs/PHYSICS_GUIDE.md)** | Rigid bodies, colliders, cloth simulation, debug panels, stress testing, and scripting integration |
| **[Scripting API Reference](docs/scripting-api.md)** | Full reference for the Rhai scripting API: transforms, input, physics, audio, timers, ECS queries |
| **[Contributing Guide](CONTRIBUTING.md)** | Guidelines for submitting issues and pull requests |

## Prerequisites

1. **Install Rust** from [rustup.rs](https://rustup.rs/) (this gives you `rustup`, `cargo`, and `rustc`)
2. Windows 10/11, Linux, or macOS
3. **Linux only:** Wayland dev libraries — `sudo apt install libwayland-dev` (Debian/Ubuntu)

### Solari / Ray Tracing (Optional)

The `solari` feature enables raytraced global illumination, DLSS Ray Reconstruction, and meshlet virtual geometry. If you don't have the required SDKs or hardware, the engine builds and runs fine without it — just don't enable the `solari` feature.

**Platform compatibility:**

| | Windows | Linux | macOS |
|---|---|---|---|
| Compile | Yes | Yes | No |
| Runtime | NVIDIA RTX only | NVIDIA RTX only | No |

DLSS is an NVIDIA-proprietary technology — the SDK only provides libraries for Windows and Linux, and requires an RTX GPU at runtime. macOS and AMD/Intel GPU users should build without this feature.

#### 1. Vulkan SDK (1.3.x or newer)

Download from [vulkan.lunarg.com](https://vulkan.lunarg.com/sdk/home) and run the installer. Any version with Vulkan 1.3+ headers works.

- **Windows:** The installer sets the `VULKAN_SDK` environment variable automatically (e.g. `C:\VulkanSDK\1.4.309.0`). No extra steps needed.
- **Linux:** `sudo apt install vulkan-sdk` or use the LunarG tarball and set `VULKAN_SDK` to the extracted directory.

Verify it's set:
```bash
echo %VULKAN_SDK%          # Windows (cmd)
echo $env:VULKAN_SDK       # Windows (PowerShell)
echo $VULKAN_SDK           # Linux/macOS
```

The build needs the `Include/` (Windows) or `include/` (Linux) directory containing `vulkan/vulkan.h`.

#### 2. DLSS SDK (v310.4.0)

Clone the exact version from NVIDIA's GitHub repo:

**Windows:**
```cmd
git clone --branch v310.4.0 https://github.com/NVIDIA/DLSS.git C:\DLSS_SDK
setx DLSS_SDK "C:\DLSS_SDK" /M
```

**Linux:**
```bash
git clone --branch v310.4.0 https://github.com/NVIDIA/DLSS.git ~/DLSS_SDK
echo 'export DLSS_SDK="$HOME/DLSS_SDK"' >> ~/.bashrc
source ~/.bashrc
```


#### 3. Build with Solari

After both SDKs are installed and environment variables are set, restart your terminal and run:

```bash
cargo run --features solari
```

### Faster Linking (Recommended)

**Windows:**
```bash
rustup component add llvm-tools-preview
```

**Linux:**
```bash
sudo apt install lld clang
```

Then create `.cargo/config.toml`:

```toml
# Windows
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"

# Linux
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

## Building

### Development

```bash
cargo run                      # run the editor (default features)
cargo run --features solari    # run with raytraced lighting + DLSS (requires SDKs)
```

### Release

Release builds disable dynamic linking for distributable binaries:

```bash
cargo release-editor    # builds static editor (~50MB) → app/release/renzora_editor.exe
cargo release-runtime   # builds static runtime (~50MB) → app/release/renzora_runtime.exe
```

Copy the runtime for exports:
```bash
cp app/release/renzora_runtime.exe runtimes/windows/   # Windows
cp app/release/renzora_runtime runtimes/linux/         # Linux
```

The `dynamic` feature (on by default) enables `bevy/dynamic_linking` for dev builds. Release aliases use `--no-default-features` and a separate `app` directory to keep static and dynamic artifacts isolated.

## Testing

Run the full test suite:

```bash
cargo test
```

Run tests for a specific module:

```bash
cargo test -- blueprint::graph_tests
cargo test -- blueprint::codegen_tests
cargo test -- blueprint::serialization_tests
cargo test -- blueprint::tests
cargo test -- scripting::tests
cargo test -- component_system::tests
cargo test -- commands::tests
cargo test -- shared::tests
cargo test -- project::tests
cargo test -- export::tests
cargo test -- theming::tests
cargo test -- docking
cargo test -- keybindings
cargo test -- file_drop
```

## Cargo Features

| Feature | Description |
|---------|-------------|
| `editor` | Full editor with UI, asset browser, scene editing |
| `runtime` | Minimal runtime for exported games |
| `physics` | Avian3D physics engine integration |
| `solari` | Raytraced GI, DLSS, and meshlet virtual geometry (requires Vulkan SDK + DLSS SDK) |
| `dynamic` | Dynamic linking for faster dev builds |
| `memory-profiling` | Memory usage tracking in diagnostics |

Default: `editor`, `physics`, `memory-profiling`

## Supported File Formats

| Format | Type |
|--------|------|
| `.glb` / `.gltf` | 3D models (meshes, materials, animations, skeletons) |
| `.obj` | 3D models (meshes) |
| `.fbx` | 3D models (meshes, skeletons) |
| `.ron` | Scene files (Bevy DynamicScene) |
| `.rhai` | Script files |
| `.blueprint` | Visual script graphs (compile to Rhai) |
| `.material_bp` | Material blueprint graphs (compile to WGSL) |
| `.particle` | Particle effect definitions |
| `.png` / `.jpg` / `.jpeg` | Textures |
| `.hdr` / `.exr` | HDR environment maps |

## Troubleshooting

### Runtime crashes immediately

Run from a terminal to see error output:
```bash
cd export_folder
./YourGame.exe
```

### Small runtime binary (~1.5MB)

Bevy was compiled with dynamic linking. Use the release alias:
```bash
cargo release-runtime
```

Output at `app/release/renzora_runtime.exe` (~50MB, statically linked).

### Release build fails with "Application Control policy has blocked this file"

If `cargo build --release` fails with OS error 4551, **Windows Smart App Control** is blocking build script executables generated during compilation. Debug builds may work because the build scripts get approved on first run and are reused by subsequent release builds.

**Fix:** Open **Windows Security → App & Browser Control → Smart App Control** and turn it off. Note that once disabled, Smart App Control cannot be re-enabled without reinstalling Windows.

**Workaround:** Run `cargo build` (debug) first, then `cargo build --release`. The debug build compiles and approves the build scripts, which are then reused by the release build.

### Export shows "Runtime not found"

Ensure the runtime binary exists at `runtimes/windows/renzora_runtime.exe`. Build it with `cargo release-runtime` and copy it there.

## License

Apache License 2.0 — see [LICENSE.md](LICENSE.md)
