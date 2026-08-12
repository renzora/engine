# Renzora Engine

A 3D game engine and visual editor built on <a href="https://bevyengine.org/" target="_blank" rel="noopener noreferrer">Bevy 0.19</a>. It's fully compatible with the Bevy plugin ecosystem and is itself modular — every system is a plugin you can add, remove, or replace. Use it as a standalone engine to build games out of the box, or treat it as a customizable foundation you can modify into your own bespoke engine.

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

> **AI-Assisted Development:** This project uses AI code generation tools (Claude by Anthropic) throughout development. If that's a concern, check out <a href="https://bevyengine.org/" target="_blank" rel="noopener noreferrer">Bevy</a>, <a href="https://godotengine.org/" target="_blank" rel="noopener noreferrer">Godot</a>, or <a href="https://fyrox.rs/" target="_blank" rel="noopener noreferrer">Fyrox</a>.

## Getting Started

**Prerequisites:** <a href="https://rustup.rs/" target="_blank" rel="noopener noreferrer">Rust</a>. Nothing else — `rust-toolchain.toml` pins the exact compiler version, so rustup fetches it for you on the first build.

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora             # build, stage dist/, and launch the editor
```

That's the whole setup. `cargo renzora` builds the workspace natively, stages a complete engine into `dist/<platform>/`, and launches the editor from it. The first build is slow; every one after that is incremental.

On **Linux** you also need the usual graphics/audio dev headers:

```bash
sudo apt install pkg-config libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libwayland-dev libxkbcommon-dev libasound2-dev libudev-dev
```

### Build Commands

| Command | What it does |
|---|---|
| `cargo renzora` | Build, stage `dist/<platform>/`, and launch the editor. |
| `cargo renzora dist` | Build and stage without launching. |
| `cargo renzora plugin <name>` | Rebuild one standalone plugin and stage it — hot reload, no editor restart. |
| `cargo renzora profile` | Profiling build with Tracy instrumentation compiled in. |
| `cargo renzora sync` | Regenerate the plugin wiring from the `renzora::add!` declarations. |
| `cargo renzora remove <crate>` | Delete a plugin crate and every reference to it. |

Building the editor always builds the runtime too. The runtime doubles as a dedicated server — run it with `--server`.

Iterate with `cargo check --profile dist` and `cargo clippy --profile dist`; run tests with `cargo test --profile dist -p <crate>`. Always pass `--profile dist` — a bare cargo command builds a second full set of artefacts under `target/debug/`, and this workspace is far too large for two of them.

Shipping a game for a platform you're not sitting at — a macOS, Android, or web build from a Windows box — is part of the editor's export system, covered in <a href="https://renzora.com/docs" target="_blank" rel="noopener noreferrer">the docs</a>.

## Documentation

Full documentation — getting started, scripting, UI, plugins, exporting, and more — lives on the website:

<strong><a href="https://renzora.com/docs" target="_blank" rel="noopener noreferrer">renzora.com/docs</a></strong>

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
| `.lua` | Scripts |
| `.blueprint` | Visual script graphs |
| `.material` | Material graphs |
| `.particle` | Particle effects |
| `.png` / `.jpg` / `.hdr` / `.exr` | Textures |
| `.ogg` / `.mp3` / `.wav` / `.flac` | Audio (native only) |
| `.rpak` | Compressed asset archives |

## License

Dual-licensed under MIT or Apache 2.0.

- <a href="LICENSE-MIT" target="_blank" rel="noopener noreferrer">MIT License</a>
- <a href="LICENSE-APACHE" target="_blank" rel="noopener noreferrer">Apache License 2.0</a>
