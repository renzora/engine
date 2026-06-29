# Asset Pipeline

How Renzora indexes, resolves, imports, and packages game assets on top of Bevy's `AssetServer`.

## The four layers

Renzora's asset handling is split across four engine crates, each with one job:

| Layer | Crate | Responsibility |
|---|---|---|
| Registry | `renzora_asset_registry` | A **metadata-only** index of every file in the project (path, kind, size, mtime). Never reads asset bytes. |
| VFS + reader | `renzora_engine` (`vfs.rs`, `asset_reader.rs`) | A virtual filesystem backed by an `.rpak` archive **or** raw disk, plus a custom Bevy `AssetReader` with a defined lookup order. |
| Import | `renzora_import` (+ `renzora_import_ui`) | Converts non-glTF 3D models to GLB at import time. |
| Scene I/O | `renzora_engine` (`scene_io.rs`) | Serializes the ECS world to RON (`.ron`) and loads it back. |

Loading the actual bytes is still Bevy's job — these layers decide *what exists*, *where to read it from*, and *what to convert it into*.

## Loading assets at runtime

Assets are loaded by path through Bevy's `AssetServer`, which returns a reference-counted `Handle<T>`:

```rust
use bevy::prelude::*;

fn load_things(asset_server: Res<AssetServer>) {
    let mesh: Handle<Scene>       = asset_server.load("models/player.glb#Scene0");
    let texture: Handle<Image>    = asset_server.load("textures/brick.png");
    let sound: Handle<AudioSource> = asset_server.load("audio/explosion.ogg");
}
```

Paths are **project-relative** (e.g. `models/player.glb`) — the custom asset reader resolves them against the archive or project directory (see below). When the last strong handle to an asset is dropped, Bevy queues it for unloading.

> Only `.glb`/`.gltf` meshes load directly at runtime. Every other 3D format is converted to GLB at **import time** — there is no runtime FBX/OBJ/USD loader.

## Asset registry — a metadata-only index

`AssetRegistryPlugin` walks the project tree **once**, on `OnEnter(SplashState::Loading)`, and records one `AssetEntry` per file. It deliberately does **not** read, decode, or instantiate anything — that stays with Bevy's `AssetServer`. The index powers the asset browser, drag-and-drop previews, and icon picking.

```rust
pub struct AssetEntry {
    pub path: String,        // project-relative, e.g. "models/player.glb"
    pub kind: AssetKind,
    pub size_bytes: u64,
    pub mtime_secs: Option<u64>,
}
```

Files are classified by lower-cased extension into one of nine coarse `AssetKind` variants:

| `AssetKind` | Extensions matched |
|---|---|
| `Model` | `glb`, `gltf`, `obj`, `fbx`, `usd`, `usda`, `usdc`, `usdz`, `abc`, `dae`, `blend` |
| `Texture` | `png`, `jpg`, `jpeg`, `bmp`, `tga`, `webp`, `hdr`, `exr` |
| `Material` | `material`, `material_bp` |
| `Scene` | `scene` |
| `Audio` | `wav`, `ogg`, `mp3`, `flac`, `opus` |
| `Video` | `mp4`, `avi`, `mov`, `webm` |
| `Script` | `rhai`, `lua`, `js`, `ts` |
| `Shader` | `wgsl`, `glsl`, `vert`, `frag`, `hlsl` |
| `Other` | everything else |

> ⚠️ **Recognition is broader than decoding.** The registry tags `.exr`, `.bmp`, `.tga`, `.webp`, `.ktx2`, `.dds`, `.js`, `.ts`, and `.opus` so they get icons in the browser — but the engine cannot actually load all of them at runtime (see [Supported formats](#supported-file-formats)). Classification ≠ a working loader.

> ⚠️ **Scene classification quirk.** `AssetKind::from_path` maps only the `.scene` extension to `AssetKind::Scene`. Renzora's real scene files are `.ron`, so the registry indexes them as `AssetKind::Other`. (The asset-browser UI does separately label `.ron`/`.scn`/`.scene` as scenes.)

## VFS and the asset reader

Two cooperating pieces decide where bytes come from.

### VFS detection (startup)

On startup `Vfs::detect()` (`renzora_engine/vfs.rs`) picks a backing store in this order:

1. `--rpak <path>` command-line override
2. An `.rpak` archive **embedded in the executable** (self-contained shipped game)
3. An adjacent `<exe-stem>.rpak` next to the executable
4. Platform bundles — Android APK assets, iOS app bundle, or WASM bytes injected from JavaScript
5. Raw filesystem (development / `--project` mode)

The detected `RpakArchive` (if any) is shared with the asset reader through the `SharedArchive` resource.

### Asset reader lookup order (per load)

`setup_asset_reader` registers a custom `EmbeddedAssetReader` **before** `DefaultPlugins`, replacing Bevy's default filesystem reader. For each `AssetServer::load(path)` it tries, in order:

1. **Absolute path** — read directly from disk
2. **Rpak archive** — the embedded/adjacent archive, if loaded
3. **Project-local directory** — `<project>/<path>` when a project is open (editor / `--project`)
4. **Exe-adjacent directory** — `<exe_dir>/<path>` for exported runtime builds
5. **CWD** — `./<path>` development fallback

This is what lets the same code path serve assets from a packed `.rpak` in a shipped game and from loose files in the project while editing — the archive simply takes priority when present.

## Importing 3D models

`renzora_import` accepts **14** model extensions (the importer list is larger than the runtime-loadable list). Everything except glTF is converted to GLB and written into the project; the engine then loads the resulting `.glb` at runtime.

| Extension(s) | Format | Import path |
|---|---|---|
| `glb`, `gltf` | glTF 2.0 | Loaded directly — no conversion |
| `obj` | Wavefront OBJ | Native converter → GLB |
| `stl` | STL | Native converter → GLB |
| `ply` | PLY | Native converter → GLB |
| `fbx` | Autodesk FBX | Via the `ufbx` crate → GLB |
| `usd`, `usda`, `usdc` | Universal Scene Description | USD submodule → GLB |
| `usdz` | USDZ (zipped USD) | USD submodule → GLB |
| `abc` | Alembic | Native converter → GLB |
| `dae` | Collada | Native converter → GLB |
| `bvh` | BioVision motion capture | **Animation only** (no mesh) |
| `blend` | Blender | Shells out to a local Blender install → GLB |

Notes:

- **`.blend`** is not parsed in-process — the importer invokes a locally installed Blender via `std::process::Command`, located through `BLENDER_PATH`, common install dirs, or `PATH`. If Blender isn't installed, `.blend` import fails.
- **`.bvh`** carries no geometry: its `convert()` always errors so the animation-extraction fallback runs instead, pulling clips out via `extract_animations_from_bvh`.

### The import overlay (`renzora_import_ui`)

Dropping a model onto the editor (or using the asset browser's **Import** button)
opens the import modal. It's a **two-pane dialog**: a left sidebar lists the
sections and a right pane shows the active one, so the modal stays a fixed size
instead of scrolling through every option at once. The modal always opens on
**Files**.

- **Files** — a drag-and-drop card (a **Browse files** button picks from disk)
  above the queued-file list. The sidebar's Files row carries a count badge of
  how many files are queued.
- **Settings** — scale, up-axis, **Flip UVs** and **Generate normals** as
  label-left / control-right rows, fed into `ImportSettings`.
- **Extract** — toggles for skeleton/skin, animations, textures and materials.
- **Optimize** — the mesh-optimization passes (vertex cache / overdraw / vertex
  fetch).
- **Destination** — a **folder tree of the project's own directories** (the same
  picker style as the marketplace install flow). Click a folder to set the
  import target; the first row, *assets (project root)*, targets the project
  root. The **Organize** radios choose a per-file `<stem>/` folder or a combined
  destination.
- **Output** — per-file import results. The sidebar row only appears once an
  import has logged results.

Clicking **Import** **dismisses the modal immediately** and hands progress off to
a **corner toast** (bottom-right): a live `[done/total]` label + progress bar
while the background worker runs, then a success/error line that auto-dismisses
after a few seconds (or via its × button). The import keeps running in the
background regardless of whether the toast is dismissed.

> Drag-and-drop with **Auto-import on drop** enabled (the default) skips the
> modal entirely and imports silently; the toast flow is the explicit
> Import-button path.

The public surface (`renzora_import`) includes `detect_format`, `supported_extensions`, `ModelFormat`, `convert_to_glb` / `convert_to_glb_with_progress`, `ImportSettings`, `UpAxis`, `optimize_glb`, and `compact_glb`, plus the `extract_animations_from_*` helpers.

```rust
use renzora_import::{detect_format, convert_to_glb, ImportSettings};

if let Some(format) = detect_format(path) {
    // Anything that isn't already GLB/glTF gets baked to a .glb beside it.
    convert_to_glb(path, &output_glb, &ImportSettings::default())?;
}
```

## Scenes — RON (`.ron`)

Scenes are saved and loaded by `renzora_engine::scene_io`. The project's default entry scene is `scenes/main.ron` (`main_scene` in `project.toml`).

`save_scene` builds a Bevy `DynamicSceneBuilder` and **denies** runtime- and editor-only components before serializing to RON — meshes, materials, cameras, Avian physics state, animation runtime state, networking components, and bevy_ui camera plumbing are all stripped, so a scene file stays a clean description of authored entities rather than a snapshot of live engine state.

`load_scene` reads the RON (through the VFS/rpak first, then disk), deserializes **lossily** (silently skipping any type not registered in this build), prunes orphaned editor-chrome UI entities, and expands nested `SceneInstance` references into their referenced scenes.

```ron
// scenes/main.ron (abridged) — a DynamicScene in RON
(
  resources: {},
  entities: {
    0: (
      components: {
        "bevy_core::name::Name": ("Player"),
        "bevy_transform::components::transform::Transform": (
          translation: (x: 0.0, y: 1.0, z: 0.0),
          rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),
          scale: (x: 1.0, y: 1.0, z: 1.0),
        ),
      },
    ),
  },
)
```

Key `scene_io` entry points: `save_scene` / `save_current_scene`, `load_scene`, `serialize_scene_to_string` / `load_scene_from_string`, and the instance/prefab helpers `spawn_scene_instance`, `expand_scene_instances`, `save_prefab_source`, `save_all_scene_instances`, and `would_create_reference_cycle`.

## Supported file formats

### Textures

| Extension | Status |
|---|---|
| `.png` | ✅ Decodes (Bevy default image features) |
| `.jpg` / `.jpeg` | ✅ Decodes (jpeg feature enabled) |
| `.hdr` | ✅ Decodes (Bevy default image features) |
| `.exr` | ❌ **Not functional** — see warning below |
| `.bmp` / `.tga` / `.webp` / `.ktx2` / `.dds` | Recognized for browser icons/thumbnails only; not enabled for runtime decode |

> ⚠️ **`.exr` is not a working texture format today.** The workspace `bevy` dependency keeps default image features (png, hdr) and adds jpeg, but **never enables the `exr` feature**; no other crate enables it either, and the thumbnail generator explicitly excludes EXR. The registry's `AssetKind` *classifies* `.exr` as a texture, but it cannot be decoded at runtime. Use `.hdr` for high-dynamic-range images.

### Audio

`.ogg`, `.mp3`, `.wav`, `.flac` — decoded by **Kira** (the engine's audio backend), **native only**. On WASM the audio crates compile to no-op stubs, so these don't decode in web builds. (The registry also tags `.opus` as audio, but it isn't in the enabled Kira feature set.)

### Scripts

| Extension | Backend |
|---|---|
| `.lua` | Lua (mlua 0.10) — **native only** |
| `.rhai` | Rhai (1.21) — **all platforms**, including WASM |
| `.js` / `.ts` | Tagged `AssetKind::Script` with a code icon, but **there is no JS/TS backend** — cosmetic recognition only |

### Other authored formats

| Extension | Contents |
|---|---|
| `.ron` | Scene (`DynamicScene` as RON) |
| `.material` | JSON-serialized `MaterialGraph` (legacy `.material_instance` / `.material_bp` still read) |
| `.blueprint` (alias `.bp`) | JSON-serialized `BlueprintGraph` (visual scripting) |
| `.particle` | RON effect definition for `renzora_hanabi` |
| `.wgsl` / `.glsl` / `.vert` / `.frag` | Shader source |
| `.html` | UI markup (parsed by `renzora_ember`'s markup runtime) |
| `.rmip` | Renzora mipmapped texture format (`renzora_rmip`) |

## The `.rpak` archive

`.rpak` is Renzora's own archive format (`renzora_rpak`), used to ship a project as one read-only blob. The asset reader serves files straight out of it without extracting to disk.

### Format (v2)

```text
[ Header — 32 bytes ]
  magic "RPAK", version (=2), flags, index_offset, index sizes
[ Data section ]
  concatenated entry payloads, each independently Stored or Zstd-compressed
[ Index section ]
  entry count + per-entry path / offset / sizes / compression / crc32
[ Footer — 16 bytes, only when appended to an executable ]
  rpak_total_size + "RPAK" magic
```

- Per-entry compression is **`Stored` or `Zstd`** — there is no LZ4, and no built-in encryption.
- An archive can stand alone (a `game.rpak` file) **or** be appended to the engine binary, detected via the trailing 16-byte footer — this is how a fully self-contained single-file game is shipped.

### Building and using archives

There is **no `renzora pack` CLI command in this repository.** Archives are produced through the `renzora_rpak` API (`RpakPacker`, `pack_project` / `pack_project_with_progress` / `pack_project_filtered`), which the editor's export tooling (`renzora_export`) drives during a build.

A dedicated server can be pointed at a stripped-down archive (one packed with `SERVER_EXTENSIONS`, dropping client-only assets) via the `--rpak` flag:

```bash
renzora --server --rpak server.rpak
```

Reading is handled by `RpakArchive` (with `BytesBackend` / `FileBackend` / `MmapBackend`); `RpakArchive::from_current_exe` detects an embedded archive, and `from_file` / `from_bytes` open standalone ones — all wired into `Vfs::detect()` automatically, so game code never touches the archive directly.
