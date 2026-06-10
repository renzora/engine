# Renzora Engine

A modular 3D game engine and editor built on [Bevy 0.18](https://bevyengine.org/), where the editor, game runtime, and dedicated server are all the same binary.

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out [Bevy](https://bevyengine.org/), [Godot](https://godotengine.org/), or [Fyrox](https://fyrox.rs/).

## Overview

Renzora is a Bevy 0.18 engine built as one large Cargo workspace in which almost every feature is its own crate and Bevy plugin. It's fully compatible with the Bevy plugin ecosystem, and it's modular all the way down — you can add, remove, or replace systems by adding or dropping plugin crates. Use it as a standalone engine and editor to build games, or treat it as a foundation you customize into your own bespoke engine.

The defining structural fact is **one binary**: the editor, the game runtime, and the dedicated server are not separate executables. They are the single `renzora` binary, with the active mode decided at runtime by command-line flags and by whether the editor bundle is present beside the exe.

## Architecture at a glance

- **One workspace binary.** The only binary in the workspace is `renzora` (`src/main.rs`). It is the engine — editor, runtime, and server in one — not a project orchestrator. It is always built runtime-shaped; there is no separate `editor` build or `editor` Cargo feature.
- **The editor is a removable cdylib bundle.** The crate `renzora_editor` compiles to `renzora_editor.dll` (Windows) / `librenzora_editor.so` (Linux) / `librenzora_editor.dylib` (macOS) and is loaded from **beside the exe** at startup. Present beside the exe → the binary runs as the editor. Delete that one file (or pass `--no-editor`) → the same binary is the shipped game.
- **~187 workspace crates.** 164 top-level crates under `crates/` plus 23 nested `editor/` subcrates. Almost every feature — each post-process effect, each editor panel, GI, physics, audio, networking — is its own plugin crate registered via `renzora::add!`.
- **The `renzora` crate is the SDK, not a CLI.** The in-repo crate literally named `renzora` is the contracts/SDK **library** (`crate-type = ["dylib", "rlib"]`). It ships as `renzora.dll`/`.so`/`.dylib` so the host binary, the dlopen'd editor bundle, and dynamic plugins all share one compiled copy and matching `TypeId`s. It is not a command-line tool.

> **Note:** Two different things share the name `renzora`. The crate in *this* repo is the SDK **library** described above. The **`renzora` CLI** — installed with `cargo install renzora` — is a separate published crate ([crates.io/crates/renzora](https://crates.io/crates/renzora)) that scaffolds projects and drives the containerized build toolchain. It's the normal way to get started; see below.

## Getting Started / Building

The quickest path is the **`renzora` CLI**, which scaffolds a project and runs every build inside the pinned `ghcr.io/renzora/engine` Docker image — so the toolchain is identical on every machine and matches the ABI the dlopen plugin system depends on. You can also build directly inside a checkout with the cargo aliases.

### Quick start (recommended)

Requires [Docker](https://docs.docker.com/get-docker/) and `git`. Install the CLI with `cargo`:

```bash
cargo install renzora      # installs the `renzora` command (a separate crates.io crate)
renzora new my-game        # clone the engine into ./my-game
cd my-game
renzora run                # build the editor in the container, then launch it (first run is slow)
```

`renzora <command>` runs builds/tests inside the container; the GPU editor and game run natively from `dist/`.

| Command | What it does |
|---|---|
| `renzora new <dir>` | Clone the engine into a new directory. |
| `renzora init` | Build/pull the toolchain image and create/start the container. |
| `renzora run [editor\|runtime]` | Build for this host and launch it (editor by default). |
| `renzora build [platforms]` | Cross-build one or more platforms (no args = all). |
| `renzora test` / `renzora check` | Run the workspace test / check suite in the container. |
| `renzora add <name> [--editor\|--dylib]` | Scaffold a plugin crate (`renzora remove <name>` deletes one). |
| `renzora upx` / `shell` / `clean` / `destroy` | Compress binaries / open a container shell / clear `target/` / remove the container. |

Under the hood the CLI runs the `docker/` scripts and the cargo aliases below, so working inside a checkout directly is equivalent.

### Working inside a checkout (cargo aliases)

With a Rust toolchain installed, the workspace ships cargo aliases in `.cargo/config.toml`:

```bash
# Build and launch the editor (first build is slow). This builds the binary
# AND the renzora_editor bundle into target/dist next to each other.
cargo renzora

# Build/launch the game runtime — same binary with --no-editor.
cargo runtime

# Run as a headless dedicated server.
cargo server
```

Other build aliases: `cargo build-editor` / `cargo build-all` (binary + editor bundle + distribution plugins, `--workspace`), `cargo build-runtime` (the lean game binary only), and `cargo build-plugins`, `cargo build-web`, `cargo build-android`, `cargo build-ios`.

### Docker (cross-platform)

Everything builds inside a container, so Docker handles all the cross toolchains (xwin, osxcross, Android NDK, wasm-bindgen, UPX, …) — no system libraries to set up, and the build is identical on every machine. The host only needs Docker; the GPU editor and game then run natively from `dist/`.

```bash
# Build every platform inside the engine-builder image.
docker run --rm -v "$PWD:/work" -w /work ghcr.io/renzora/engine \
  docker/build-all.sh dist

# Or pass a subset of platform tokens.
docker/build-all.sh dist linux windows wasm
```

`docker/build-all.sh` platform tokens: `linux`, `windows`, `macos` (= x64 + arm64), `macos-x64`, `macos-arm64`, `wasm`, `android` (= arm64 + x86), `android-arm64`, `android-x86`, `ios`. macOS builds only when osxcross is present; the android/ios lanes are best-effort and don't fail the build.

Helper scripts in `docker/`:

| Script | What it does |
|---|---|
| `docker/build-all.sh <out-dir> [platform…]` | Cross-build one or more platforms (no platform args = all). |
| `docker/add-plugin.sh <name> [--editor\|--dylib]` | Scaffold a new plugin crate (`--editor` = editor-scope dep; `--dylib` = cdylib distribution plugin). Flags are mutually exclusive. |
| `docker/remove-plugin.sh <name>` | Delete a plugin crate. |
| `docker/upx-compress.sh [platform\|path…]` | UPX-compress built binaries. |

> **Note:** `docker/Dockerfile` (`FROM rust:1.93.0-bookworm`) is the single source of truth for the Rust version — there is no `rust-toolchain.toml`.

## Run Modes

The same `renzora` binary selects its mode at runtime:

| Invocation | Mode |
|---|---|
| *(default windowed launch)* | Editor if `renzora_editor` is beside the exe, otherwise the shipped game. |
| `--no-editor` (or `RENZORA_NO_EDITOR`) | Force game mode even when the editor bundle is present. |
| `--server` | Headless dedicated server (no GPU, no window; runs at the network tick). |
| `--host` | Windowed listen server — client + server in one process. Wins over `--server` if both are passed. |

A `--server` or `--host` launch is never an editor session, even if the bundle is present.

Server / host configuration flags (these overlay `[network]` in `project.toml`):

| Flag | Meaning | Default |
|---|---|---|
| `--port <n>` | Listen port | `7636` |
| `--addr` / `--address <ip>` | Bind address | — |
| `--tick-rate <n>` | Network tick rate (Hz) | `64` |
| `--max-clients <n>` | Maximum connected clients | `32` |

## Build Output Layout

`docker/build-all.sh` writes arch-suffixed directories. Desktop binaries land directly in their directory; web and mobile artifacts nest under `runtime/`:

| Platform | Output |
|---|---|
| Linux x64 | `dist/linux-x64/` |
| Windows x64 | `dist/windows-x64/` |
| macOS x64 | `dist/macos-x64/` |
| macOS arm64 | `dist/macos-arm64/` |
| Web (WASM) | `dist/web-wasm32/runtime/` |
| Android arm64 | `dist/android-arm64/runtime/` |
| Android x86 | `dist/android-x86/runtime/` |
| iOS arm64 | `dist/ios-arm64/runtime/` |

The web build is **game-runtime only** (WebGPU) — there is no WASM editor.

## Documentation

Full guides — plugin development, the scripting API, the `.html` UI/markup language, networking, and more — live at **<https://renzora.com/docs>**.

## Supported Platforms

| Platform | Devices |
|----------|---------|
| Windows x64 | Desktop, PCVR (OpenXR) |
| Linux x64 | Desktop, Steam Deck |
| macOS | Intel + Apple Silicon |
| Web (WASM) | WebGPU-capable browsers (Chrome/Edge 113+) |
| Android ARM64 | Phones, tablets, standalone XR headsets |
| iOS | iPhone, iPad |

> **Note:** XR is provided via the vendored `bevy_oxr` (OpenXR) stack. There is **no tvOS / Apple TV** target — the build toolchain installs no tvOS Rust target and there is no tvOS build lane.

## Supported File Formats

The engine distinguishes between formats that **load directly at runtime** and formats that are **import-time only** (converted on import and written into the project).

### 3D models

| Format | Support |
|---|---|
| `.glb` / `.gltf` | Load directly at runtime. |
| `.obj`, `.stl`, `.ply`, `.fbx`, `.usd`, `.usda`, `.usdc`, `.usdz`, `.abc`, `.dae` | **Import-time only** — converted to GLB on import. |
| `.bvh` | **Import-time only**, animation-only. |
| `.blend` | **Import-time only** — converted via a locally installed Blender. |

> Only `.glb` / `.gltf` are loaded by the runtime. Everything else in the table is handled by the importer and baked to GLB; it is not a runtime-loadable format.

### Textures

| Format | Support |
|---|---|
| `.png`, `.jpg`, `.hdr` | Load at runtime. |
| `.bmp`, `.tga`, `.webp`, `.ktx2`, `.dds` | Recognized for thumbnails/icons only; not decoded at runtime. |

> `.exr` is **not** a working texture format today — the EXR image feature is not compiled in.

### Scripts and visual scripting

| Format | Support |
|---|---|
| `.lua` | Lua 5.4 (mlua), **native only** — not available on WASM. |
| `.rhai` | Rhai, pure Rust, runs **everywhere including WASM**. A subset of the Lua API. |
| `.blueprint` (`.bp`) | Visual node graph, interpreted directly at runtime. |

Backends dispatch by file extension; both Lua and Rhai ship in the runtime. See the [scripting docs](https://renzora.com/docs) for the per-backend function reference.

### Shaders and UI

| Format | Support |
|---|---|
| `.wgsl`, `.glsl` (`.vert` / `.frag`) | Shader source. |
| `.html` | UI markup (rendered by `renzora_ember`, with `{{ }}` reactive bindings). |

### Scenes and authoring

| Format | Support |
|---|---|
| `.ron` | Scene files (serialized ECS world). |
| `.material` | Material graph (JSON). |
| `.particle` | Particle effect definition (RON). |

### Audio (native only)

| Format | Support |
|---|---|
| `.ogg`, `.mp3`, `.wav`, `.flac` | Decoded via Kira — **native only** (no audio on WASM). |

### Archives

| Format | Support |
|---|---|
| `.rpak` | Renzora's own v2 asset archive (per-entry Stored or Zstd compression; can be embedded in the exe). |

## License

Dual-licensed under MIT or Apache 2.0.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)
