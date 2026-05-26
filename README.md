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

## Extending the Engine

Renzora is plugin-based — you can add components, editor panels, gameplay, and networking in Rust, and script behavior in Lua/Rhai. See **[Plugin Development](docs/plugin-development.md)** for the SDK, scaffolding, components, and scripting.

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
