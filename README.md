# Renzora Engine

A 3D and 2D game engine and visual editor built on <a href="https://bevyengine.org/" target="_blank" rel="noopener noreferrer">Bevy 0.20</a>. It's fully compatible with the Bevy plugin ecosystem and is itself modular: every system is a plugin you can add, remove, or replace. Use it as a standalone engine to build games out of the box, or treat it as a customizable foundation you can modify into your own bespoke engine.

![Renzora Editor](assets/previews/interface.png)

> **Warning:** Early alpha. Expect bugs, incomplete features, and breaking changes between versions.

## Features

### Editor

- Dockable panels with tabs, split panes and floating windows, saved per layout
- Up to four viewports, each with its own camera, render mode and render resolution
- Hierarchy, inspector, asset browser and console, with drag and drop between them
- Translate, rotate and scale gizmos, an infinite grid, and camera angle presets
- Undo and redo across the editor, with autosave
- Command palette, rebindable shortcuts, and switchable themes and fonts
- Built-in code editor for scripts and shaders
- Play runs the game as its own process in the Game panel, and live link keeps edits flowing both ways
- Guided tutorial for first-time users
- UI scale and DPI support
- Runs on the web as well as the desktop

### Rendering

- Physically based materials with clearcoat, anisotropy, transmission and parallax displacement
- Node-based material editor that generates WESL shaders, for 3D meshes and 2D sprites
- Shader editor, plus support for shaders written in the legacy WGSL dialect
- Solari ray-traced global illumination with its own denoiser and ray-traced glass
- DLSS upscaling and ray reconstruction
- Virtual geometry: `.rvg` models stream in clusters of triangles for very dense scenes
- Gaussian splatting
- Point, spot, directional and rectangular area lights, with shadows and contact shadows
- Environment maps, skyboxes, light probes, parallax-corrected reflection probes and planar reflections
- Physical atmosphere, volumetric clouds, volumetric fog and distance fog
- SSAO, SSR, bloom, auto exposure, depth of field, motion blur and tonemapping
- MSAA, FXAA, SMAA, TAA and contrast-adaptive sharpening
- Order-independent transparency and forward decals
- Around 30 shader-file post effects, including color grading, chromatic aberration, vignette, film grain, outline, CRT and toon
- Graphics quality tiers (Low, Medium, High)
- Mesh LOD with distance crossfade
- Texture streaming and the compact `.rmip` texture format
- Recovers from GPU device loss instead of crashing

### World building

- Terrain sculpting and painting
- Foliage painting
- Spectral FFT ocean water
- Wind that sways foliage and meshes
- Tilemaps with composite multi-tile objects and randomised painting
- Mesh modelling tools
- Shape library and level presets

### 2D

- Sprites, sprite animation clips and a dedicated 2D view
- 2D lighting
- 2D physics with Y-sorting

### Animation

- Animator with blend graphs, masks, transitions and animation events
- Animation retargeting and root motion
- Property animation of any field, with per-key easing
- Morph targets and facial animation
- Rigging tools and inverse kinematics
- FBX animation import

### Physics and simulation

- Pluggable physics backends; Avian 3D and 2D are bundled
- Ragdolls
- Cloth
- Hair strands
- Parkour traversal and character locomotion
- Navmesh pathfinding
- GPU particle system with an effect editor and a library of ready-made effects

### Scripting

- Rust scripts compiled as native plugins with full `&mut World` access, recompiled on save
- Pluggable language backends: install a language from the marketplace, and several can share one project
- Hooks for ready, update, UI, drawing, RPC, animation events, HTTP and player join/leave
- Rebindable input action map for keyboard, mouse and multiple gamepads
- Domain APIs for physics, animation, IK, particles, parkour, ragdolls, navigation and game UI
- Engine crates add new script functions declaratively, so every language gets them

### Audio

- Own audio backend with a mixer, buses and effects
- Spatial audio with distance falloff
- Pluggable audio backends

### UI

- Ember, Renzora's UI toolkit, used for both the editor and in-game UI
- Game UI editor with markup templates
- Rich text, font switching and font weights
- SVG images anywhere an image is accepted

### Multiplayer (alpha)

- Client and headless dedicated server from one binary (`--server`)
- Replication and RPC over UDP
- Network panel in the editor

### XR

- Run games in a VR headset over OpenXR (Quest Link, SteamVR), with a desktop mirror
- Headset editing mode (`cargo renzora xr`)

### Plugins and extending

- Every engine system is a plugin you can remove
- Native plugins ship as source and compile on the machine that installs them
- Bundled plugins are compiled into the binary and dropped by deleting a folder
- Custom editor panels, inspectors, material nodes, post effects, and audio, physics, network and script backends
- Localization of the editor and games

### Assets and scenes

- BSN scene files with prefabs and thumbnails
- Import of glTF, FBX, OBJ, STL and PLY, with GLB compaction
- Hot reload of project files
- `.rpak` compressed asset archives

### Exporting

- One binary is both the editor and the game: leave out the editor image and it is the shipped game
- Lean exports that leave out every feature the project doesn't use
- Export to Windows, Linux, macOS, Android, iOS and the web, including cross-compiling from one machine
- Built-in updater

### Marketplace

- Browse and install plugins, scripts and assets inside the editor
- Publish your own from the editor's Publish panel
- Creator credits

## Planned

This list changes as work lands. See `RELEASE_NOTES.md` for what each nightly actually contains.

### Next up

- Touch and gesture input for mobile games
- A fixed-timestep script hook (`on_fixed_update`)
- Shadow quality controls: cascades, bias and soft shadows
- Controller navigation for game menus, and accessibility (screen readers)
- IME text entry for Chinese, Japanese and Korean input
- GPU occlusion culling, clustered decals, light cookies and irradiance volumes
- A save-game API for scripts

### In progress

- Multiplayer: client prediction, a secure handshake, avatar spawning and more transports
- Facial rigging and posing
- Code editor: minimap, code folding and undo history
- One shared environment and lighting setup across multiple viewports
- A unified world environment component
- Translating the remaining editor text

### Later

- Cinematic sequencer for cutscenes
- Behavior trees and AI perception
- Crowd avoidance for navmesh agents
- Camera rigs with shake, blending and rails
- Spline mesh tools for roads, pipes and fences
- Vertex and mesh painting
- Vehicle physics
- Data tables
- Lightmap baking
- Level streaming for large worlds
- In-editor profiler
- HDR display output and video capture

### Following Bevy 0.21

- Sprite rendering moving to `SpriteMesh`
- Ambient lights folding into environment-map lights
- Assets becoming entities
- Upstream `.bsn` file format and entity inspector

## Getting Started

**Prerequisites:** <a href="https://rustup.rs/" target="_blank" rel="noopener noreferrer">Rust</a>.

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
| `cargo renzora profile` | Profiling build with Tracy instrumentation compiled in. |
| `cargo renzora sync` | Regenerate the plugin wiring from the `renzora::add!` declarations. |
| `cargo renzora remove <crate>` | Delete a plugin crate and every reference to it. |

Building the editor always builds the runtime too. The runtime doubles as a dedicated server: run it with `--server`.

Iterate with `cargo check --profile dist` and `cargo clippy --profile dist`; run tests with `cargo test --profile dist -p <crate>`. Always pass `--profile dist`. A bare cargo command builds a second full set of artefacts under `target/debug/`, and this workspace is far too large for two of them.

Shipping a game for a platform you're not sitting at (a macOS, Android, or web build from a Windows box) is part of the editor's export system, covered in <a href="https://renzora.com/docs" target="_blank" rel="noopener noreferrer">the docs</a>.

## Documentation

Full documentation (getting started, scripting, UI, plugins, exporting, and more) lives on the website:

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
| `.rvg` | Virtual geometry models |
| `.bsn` / `.ron` | Scene files (`.ron` is the legacy format) |
| `.rs` / `.lua` | Scripts (Lua through a marketplace language plugin) |
| `.material` | Material graphs |
| `.wesl` / `.wgsl` | Shaders and post effects (`.wgsl` in the legacy dialect is translated) |
| `.anim` | Animation clips |
| `.vfx` / `.particle` | Particle effects (`.particle` is the legacy format) |
| `.png` / `.jpg` / `.hdr` / `.exr` / `.svg` | Textures and images |
| `.rmip` | Streamed, compressed textures |
| `.ogg` / `.mp3` / `.wav` / `.flac` | Audio (native only) |
| `.rpak` | Compressed asset archives |

## License

Dual-licensed under MIT or Apache 2.0.

- <a href="LICENSE-MIT" target="_blank" rel="noopener noreferrer">MIT License</a>
- <a href="LICENSE-APACHE" target="_blank" rel="noopener noreferrer">Apache License 2.0</a>
- <a href="https://github.com/renzora/engine/blob/main/docs/r1-alpha8/contributing/ai-policy.md" target="_blank" rel="noopener noreferrer">AI Policy</a>
