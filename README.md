# Renzora Engine

A 3D game engine and visual editor built on [Bevy 0.18](https://bevyengine.org/). Currently in **alpha** — actively developing toward feature parity with Bevy's full capabilities.

> **Warning:** This engine is in early alpha. You will encounter bugs, incomplete features, and unexpected behavior. APIs and file formats may change without notice between versions.

## Status

**Alpha** — Core systems are functional and the editor is usable for scene composition, scripting, and game export. Not yet recommended for production use.

**277 unit tests** across 14 modules covering blueprint graphs, codegen, serialization, scripting, component registry, UI docking, theming, export, physics data, and more.

## Prerequisites

1. **Install Rust** from [rustup.rs](https://rustup.rs/) (this gives you `rustup`, `cargo`, and `rustc`)
2. Windows 10/11, Linux, or macOS
3. **Linux only:** Wayland dev libraries — `sudo apt install libwayland-dev` (Debian/Ubuntu)

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

Debug builds use dynamic linking for fast iteration:

```bash
cargo run       # run the editor
cargo build     # build the editor
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
| `dynamic` | Dynamic linking for faster dev builds (default) |

Default: `editor`, `dynamic`

---

## Feature Parity with Bevy 0.18

Progress toward exposing Bevy's capabilities through the editor and runtime. Percentages reflect how much of each area is usable end-to-end (editor UI + serialization + runtime).

### Rendering — 75%

| Feature | Status | Notes |
|---------|--------|-------|
| PBR materials | Done | Base color, metallic, roughness, normal maps, emissive |
| Standard mesh primitives | Done | Cube, sphere, cylinder, plane |
| GLTF/GLB import | Done | Full hierarchy, meshes, materials, animations, drag-and-drop |
| Point lights | Done | Color, intensity, range, radius, shadows |
| Directional lights | Done | Color, illuminance, shadows |
| Spot lights | Done | Inner/outer cone angle, range, shadows |
| Sun light | Done | Azimuth/elevation angles, angular diameter, shadows |
| Ambient light | Done | Global color and brightness |
| Shadows | Done | All light types support shadow casting |
| Meshlet / virtual geometry | Done | GPU-driven rendering, automatic LOD, occlusion culling |
| Solari raytraced lighting | Done | GPU raytracing with DLSS Ray Reconstruction |
| Sprite 2D | Done | Texture, color tint, flip, anchor, sprite sheets with named animations |
| Custom material shaders | Done | Visual blueprint editor compiles to WGSL |
| HDR environment maps | Done | Equirectangular panorama skybox with rotation and energy |
| Procedural sky | Done | Gradient sky with customizable colors and curves |
| Volumetric clouds | Done | Procedural clouds with coverage, density, wind, altitude |
| Transparency / alpha modes | Partial | Blend mode on materials, not all modes exposed in UI |
| Multi-camera rendering | Partial | Camera preview exists, multi-viewport not fully wired |
| GPU instancing | Not started | Bevy supports it, not exposed in editor |
| Morph targets | Not started | |

### Post-Processing — 90%

| Feature | Status | Notes |
|---------|--------|-------|
| Bloom | Done | Intensity and threshold |
| Tonemapping | Done | 7 modes (Reinhard, ACES, AgX, TonyMcMapface, BlenderFilmic, etc.) + EV100 |
| SSAO | Done | Intensity and radius |
| SSR (screen-space reflections) | Done | Intensity and max steps |
| Fog | Done | Color, start/end distance |
| Anti-aliasing | Done | MSAA + FXAA |
| Depth of field | Done | Focal distance and aperture |
| Motion blur | Done | Intensity toggle |
| TAA | Not started | Bevy has it, not yet exposed |
| Chromatic aberration | Not started | |

### Physics — 80%

| Feature | Status | Notes |
|---------|--------|-------|
| Rigid bodies | Done | Dynamic, static, kinematic |
| Box collider | Done | Half-extents, offset, friction, restitution |
| Sphere collider | Done | Radius |
| Capsule collider | Done | Radius + half-height |
| Cylinder collider | Done | Radius + half-height |
| Sensor / trigger zones | Done | Boolean flag on colliders |
| Raycasting | Done | Full API from scripts |
| Mass & damping | Done | Mass, gravity scale, linear/angular damping |
| Axis constraints | Done | Lock X/Y/Z translation and rotation |
| Collision layers | Partial | Layer system exists, UI not fully exposed |
| Joints / constraints | Not started | |
| Mesh colliders | Not started | |

### Animation — 70%

| Feature | Status | Notes |
|---------|--------|-------|
| GLTF animation playback | Done | Clip names, play/pause, speed, clip selection |
| Editor keyframe animation | Done | Timeline, per-property tracks, linear/step/bezier interpolation |
| Sprite sheet animation | Done | Frame-based with named clips and looping |
| Animation graph | Partial | Bevy graph integration exists, blending not exposed |
| Skeletal animation blending | Not started | |
| IK | Not started | |

### Scripting — 85%

| Feature | Status | Notes |
|---------|--------|-------|
| Rhai scripting | Done | 16 API modules: transform, input, math, physics, audio, time, debug, environment, rendering, animation, camera, components, scene, particles, ECS |
| Script properties | Done | `props()` function exposes variables in inspector |
| Blueprint visual scripting | Done | 50+ node types, compiles to Rhai |
| Material blueprints | Done | Node graph compiles to WGSL shaders |
| Gamepad input | Done | Full stick/trigger/button support |
| Hot reload | Partial | Script recompilation on save, no live state preservation |

### Scene & Editor — 85%

| Feature | Status | Notes |
|---------|--------|-------|
| Scene save/load (.ron) | Done | Reflection-based serialization |
| Scene instance nesting | Done | Reference other scene files as instances |
| Multi-scene tabs | Done | Work on multiple scenes simultaneously |
| Hierarchy panel | Done | Drag-and-drop, visibility, locking, search |
| Inspector panel | Done | Auto-reflect + custom inspectors per component |
| Asset browser | Done | OS-like folders, previews, drag-and-drop import |
| Transform gizmos | Done | Translate, rotate, scale in world/local space |
| Undo/redo | Done | Command-based history system |
| Customizable keybindings | Done | Full rebinding UI |
| Docking layout system | Done | Drag panels, save/load workspace layouts |
| Console | Done | Multi-level logging with filters |
| Play mode | Done | Edit/play toggle with script execution |
| Grid | Done | 3D and 2D editor grids |
| Settings panel | Done | Editor preferences |
| Theme system | Done | Dark theme with customizable color groups |
| Plugin system | Partial | Plugin API exists, command execution not complete |

### Terrain — 70%

| Feature | Status | Notes |
|---------|--------|-------|
| Chunk-based terrain | Done | Configurable grid (4x4, 8x8, 16x16), per-chunk height data |
| Terrain sculpting | Done | Brush tools with configurable strength and size |
| Terrain materials | Done | Material binding per terrain |
| Terrain LOD | Not started | |
| Texture splatting | Not started | |

### Particles — 80%

| Feature | Status | Notes |
|---------|--------|-------|
| Hanabi integration | Done | Full effect builder |
| Emit shapes | Done | Point, circle, sphere, cone, rect, box |
| Spawn modes | Done | Rate, burst, burst-rate |
| Velocity modes | Done | Directional, radial, tangent, random |
| Color gradients | Done | Gradient stops and curves |
| Blend modes | Done | Additive, alpha, opaque |
| Particle editor panel | Done | Visual editor with live preview |
| Particle asset loading | Partial | Inline definitions work, file-based loading incomplete |

### 2D — 60%

| Feature | Status | Notes |
|---------|--------|-------|
| 2D camera | Done | Orthographic projection |
| Sprite rendering | Done | Textures, color, flip, anchor |
| Sprite sheet animation | Done | Named clips, frame control |
| 2D viewport | Done | Editor viewport for 2D scenes |
| UI nodes | Done | Panel, label, button, image |
| 2D physics | Not started | |
| Tilemaps | Not started | |

### Export — 85%

| Feature | Status | Notes |
|---------|--------|-------|
| Game packaging | Done | Single-executable output |
| Asset bundling (RPCK v2) | Done | zstd compression, automatic asset discovery |
| Windows target | Done | |
| Linux target | Done | |
| macOS target | Done | x86_64 and ARM |
| Standalone runtime | Done | Minimal runtime without editor dependencies |
| Web (WASM) | Not started | |

### Audio — 40%

| Feature | Status | Notes |
|---------|--------|-------|
| Play/stop sounds | Done | Via scripting API |
| Spatial audio | Done | Via scripting API |
| Volume control | Done | Via scripting API |
| Audio listener component | Done | Registered component |
| Audio editor / mixer | Not started | No dedicated UI panel |
| Audio asset preview | Not started | |

### Diagnostics — 90%

| Feature | Status | Notes |
|---------|--------|-------|
| Performance profiler | Done | System timing |
| ECS stats | Done | Entity and component counts |
| Memory profiler | Done | Usage trending |
| Render stats | Done | Draw calls, frame metrics |
| Physics debug | Done | Collider visualization |
| Camera debug | Done | Frustum visualization |
| Gamepad debug | Done | Input state monitor |
| Debug drawing (scripting) | Done | Lines, spheres, boxes, arrows with colors |

### Overall Progress: ~75%

The engine covers the majority of Bevy 0.18's core rendering, physics, and scripting capabilities. The main gaps are in advanced animation (skeletal blending, IK), 2D-specific features (tilemaps, 2D physics), web export, and some rendering features not yet exposed through the editor UI.

---

## Supported File Formats

| Format | Type |
|--------|------|
| `.glb` / `.gltf` | 3D models (meshes, materials, animations, skeletons) |
| `.ron` | Scene files (Bevy DynamicScene) |
| `.rhai` | Script files |
| `.blueprint` | Visual script graphs |
| `.material_bp` | Material blueprint graphs |
| `.effect` | Particle effect definitions |
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

### Export shows "Runtime not found"

Ensure the runtime binary exists at `runtimes/windows/renzora_runtime.exe`. Build it with `cargo release-runtime` and copy it there.

## License

Apache License 2.0 — see [LICENSE.md](LICENSE.md)
