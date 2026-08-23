# Asset Pipeline

How Renzora indexes, resolves, imports, and packages game assets on top of Bevy's `AssetServer`.

## The four layers

Renzora's asset handling is split across four engine crates, each with one job:

| Layer | Crate | Responsibility |
|---|---|---|
| Registry | `renzora_asset_registry` | A **metadata-only** index of every file in the project (path, kind, size, mtime). Never reads asset bytes. |
| VFS + reader | `renzora_engine` (`vfs.rs`, `asset_reader.rs`) | A virtual filesystem backed by an `.rpak` archive **or** raw disk, plus a custom Bevy `AssetReader` with a defined lookup order. |
| Import | `renzora_import` (+ `renzora_import_ui`) | Converts non-glTF 3D models to GLB, and copies every other permitted asset (images, audio, `.bsn`, `.particle`, `.material`, fonts, scripts) into the project at import time. |
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
- **`.stl`** is geometry only — a bag of triangles with a facet normal each, and no hierarchy, names, materials, textures, UVs or units. The converter synthesises the rest: one neutral placeholder material, and a **box-projected UV set** taken from whichever axis each vertex's normal faces most strongly, normalised across the model's bounds. Writing the all-zero UVs it used to would leave the mesh *claiming* to have coordinates, so any texture assigned later sampled a single texel and rendered as a flat block of colour with nothing to explain why. The projection is not a real unwrap — corners seam — but it is a usable starting point. **Flip UVs** inverts it, exactly as it does a real UV set. See [Sibling texture sets](#sibling-texture-sets) for wiring up the `textures/` folder such a model usually ships with.

### Units and orientation

Every converted GLB comes out in the engine's convention: **metres, Y-up,
right-handed**, with each object standing where the source file placed it. A
centimetre, Z-up scene does not need a scale or rotation fixed up by hand after
import.

For FBX this is `ufbx`'s job — `load_scene` asks it for `right_handed_y_up` at
one metre per unit — but the conversion does not always land in the vertex data.
ufbx can only rewrite the vertices when a mesh has a single placement it is free
to modify; in a scene export, where hundreds of nodes each carry their own
placement, it puts the conversion into the **node transforms** instead. The
importer therefore bakes each instance's `geometry_to_world` into the vertices it
writes, rather than emitting raw geometry-space positions. Skipping that step is
what made a centimetre, Z-up building exterior import 100× oversized, lying on
its side, with every prop stacked at the origin.

### Legacy materials are not PBR materials

ufbx presents every FBX material through one normalized PBR view, whatever the
source shader was. That is convenient, but it means a legacy Phong material's
extended slots get filled from whichever Phong property is nearest — and the
values do not survive the mapping.

`TransparencyFactor` is the one that bites. It lands in `transmission_factor`,
and Phong transparency is `TransparentColor * TransparencyFactor`, so the
near-universal spelling of *opaque* — black transparent colour, factor `1.0` —
reads back as transmission `1.0`. Taken at face value it turned all 132
materials of a building exterior into fully transmissive glass: the scene
rendered milky with the sky bleeding through it, and signage read mirrored
because you were seeing each surface's back face through its own transparent
front.

So the importer gates the extended channels on `material.shader_type`. A
`FbxPhong`, `FbxLambert`, `BlenderPhong`, `WavefrontMtl` or `Unknown` material
has no clearcoat, transmission or anisotropy to describe, and gets the glTF-spec
defaults; its transparency comes from `opacity`, which ufbx derives properly.
Only a real PBR shader — StingrayPBS, Arnold, 3ds Max Physical, OpenPBR,
`GltfMaterial` — has those slots read.

### Attributes are per corner, not per vertex

FBX stores UVs and normals against **mesh corners**, so a vertex on a UV seam or
a hard edge carries a different value in each face that meets there. glTF has no
such concept — its attributes are already per vertex, because the exporter split
them — which is why a `.glb` and an `.fbx` of the same scene are not the same
problem.

Reading one value per vertex (`vertex_uv[vertex_first_index[v]]`) silently
rewrites every seam vertex to whichever face happened to be visited first. On a
building exterior that is **26.8% of vertices given the wrong UV** and 16.1%
given the wrong normal: textures slide off the surfaces they belong to, and hard
edges shade as though they were smooth.

So the importer builds one entry per corner and hands the streams to
`ufbx::generate_indices`, upstream's helper for exactly this, which collapses
identical tuples in place and returns the unique count. Splitting only where an
attribute genuinely differs costs 1.38× the source vertex count on that scene,
against the 4.09× that emitting every corner as its own vertex would.

Two consequences worth knowing:

- **Instanced meshes are expanded.** A mesh referenced by several nodes is
  written once per node, each with its own placement baked in. GLB node
  instancing is not preserved.
- **Skinned meshes stay in geometry space.** Their inverse bind matrices come
  from `cluster.geometry_to_bone`, which is defined *from* that space, so baking
  the node transform into the vertices would apply it twice the moment a clip
  played. Rigged characters are unaffected by the placement pass.

`ImportSettings::scale` feeds ufbx's `target_unit_meters`, so it names the
metres-per-unit you want out, and `ImportSettings::up_axis` (`Auto` by default)
covers the formats — Collada, Alembic, Blender — that carry an explicit up-axis
the importer reads itself.

Where a format carries no axis at all, `Auto` falls back to that format's
convention rather than to "leave it alone". **STL** is the case that matters:
it is a CAD and 3D-printing interchange format whose build plate is the XY
plane, so every producer writes +Z up, and `Auto` rotates accordingly. Picking
`Y-Up` explicitly still overrides it. OBJ, PLY and Alembic are conventionally
Y-up already, so `Auto` leaves those untouched.

The auto-detected scale is re-read for each new import queue. It is deliberately
not carried over from the last one: a detected value describes the file it came
from, and inheriting a centimetre USD's `0.01` would silently shrink the next
import a hundredfold — including formats like STL that store no units at all.
Typing a scale yourself pins it, and it then survives until you change it.

### Sibling texture sets

A geometry-only model almost always ships with a `textures/` folder beside it,
because the format has nowhere to record what those files are for. The importer
finds them, groups them into **sets**, and offers the sets as a **Textures**
dropdown in the import inspector's Import settings. Pick one — the window
reconverts on its own — and its maps are bound to the model's material and baked
to `.rmip` like any other texture. The material takes the set's name, so `Steel` is findable in the
material browser in a way `Default` is not.

Grouping strips a role suffix from each filename and clusters what's left, with
a longer stem folding into a shorter one it's a prefix of:

```
textures/KSR29sniperrifle_Base_Color.jpg                      ┐
textures/KSR29sniperrifle_Normal_OpenGL.jpg                   ├ set "KSR29sniperrifle"
textures/KSR29sniperrifle_Roughness.jpg                       │   4 maps
textures/KSR29sniperrifle_low_Material.005_AmbientOcclusion   ┘
textures/Sniper_KSR_29_Col.jpg                                ┐
textures/Sniper_KSR_29_nor.jpg                                ├ set "Sniper_KSR_29"
textures/Sniper_KSR_29_spec.jpg                               ┘   3 maps
textures/SKY.jpg                                              → no role, not a set
```

Role suffixes cover the long forms (`base_color`, `roughness`, `normal_opengl`,
`ambientocclusion`) and the single-letter convention (`Steel_C` / `_N` / `_S`),
but only when the suffix is its own token — so `manor` is not a normal map and
`Residential Buildings 001` is not a set.

**Nothing is bound automatically**, and that is deliberate. Real packs ship
competing sets in one folder: two full PBR sets for one rifle, or five surface
materials shared across ten buildings. Picking "the base colour" out of four
would be wrong most of the time and would *look* deliberate, which is worse than
leaving it alone.

The inspector adds one caveat whenever a set is bound, because it decides the
outcome and the importer cannot resolve it: the UVs were projected, so a
**tileable** surface map lines up and a map **baked for a specific unwrap** does
not. Nothing in the file can recover the original layout — those maps belong to
whatever model the pack exported the STL from. Detecting which kind an image is
was tried and dropped: an edge-continuity test read packed atlases as seamless
often enough to be useless, and a wrong verdict is worse than none.

Only formats that store no materials of their own consult this. A model that
names its own textures is never overridden by a folder full of guesses.

### One pipeline, whatever the source format

Every format converts to a GLB and then runs the **same** pass over it:

```
FBX / OBJ / USD / Collada / Alembic / STL / PLY
        ↓  format-specific: geometry, materials, where each texture lives
      GLB
        ↓  shared: texture roles → write .rmip → memory budget → read materials
   ImportResult
```

glTF and GLB sources skip the first step, since they're already a GLB. A `.blend`
is exported by Blender to a GLB out-of-process and enters the same way.

A converter is responsible for exactly two things beyond geometry:

1. **Complete glTF materials.** `gltf_json`'s typed `Material` only covers the
   metal-rough core, so the writer patches the JSON directly to add the name,
   emissive, occlusion, alpha mode and the `KHR_materials_*` extensions. Two
   channels glTF has no home for — a separate opacity or specular map, and the
   separate roughness/metallic maps the PBR-MTL extension defines — ride in a
   `RENZORA_materials_legacy` vendor extension.
2. **Where each texture lives.** Embedded images go into the GLB's binary chunk
   as `bufferView`-backed images; referenced ones get an absolute path as the
   image `uri`, which the shared pass rewrites once it has processed the file.
   Locating that file is format-specific (FBX in particular records three
   unreliable variants of the path); everything after it is not.

This split is recent and worth knowing about, because the bugs it fixed all had
the same shape. Each converter used to do its own role scanning, texture
extraction and material extraction, so a fix or a safeguard added to one format
simply didn't exist in the others — FBX ended up the only format with no
resolution clamp and no `.rmip` output at all, and it wrote a single primitive
referencing material 0 no matter how many materials the source had.

### Textures and materials

A source file supplies its textures one of two ways, and the importer handles
both:

- **Embedded** — the image bytes are stored inside the file. They're written out
  to `<model_dir>/textures/<name>.<ext>`.
- **Referenced** — the file names an image sitting beside it, which is what most
  real-world FBX does. The importer resolves the reference and **copies** the
  file into the same `textures/` folder. Copied, not buffered: a single scene
  can reference well over a gigabyte of external maps, and reading all of it
  into memory just to write it back out is a good way to run a machine out of
  RAM.

Resolving a reference means trying, in order, the absolute path recorded at
export time, the recorded relative path re-joined against the model's own
directory, and finally the bare filename in that directory and in a `textures/`
subfolder. Paths are re-split on both separators, so a Windows-authored FBX
resolves on Linux and vice versa. Anything still unfound is reported as a single
import warning naming the count and the first missing file, rather than one line
per texture.

**One primitive per material, each with its own vertices.** A glTF primitive
wears exactly one material, so the converters bucket triangles by their source
material and emit a primitive per bucket. Faces the source left unassigned
collect in a final primitive with no material. Previously everything was merged
into a single primitive pointing at material 0, so a scene with a hundred
materials rendered entirely in the first one.

The second half of that sentence — *each with its own vertices* — is the part
that is easy to get wrong and expensive to get wrong. glTF lets several
primitives share one attribute accessor, and nothing about the file looks
unusual when they do. But **Bevy builds one `Mesh` asset per primitive and reads
that primitive's accessors in full**, so a shared accessor is not shared in
memory: it is copied once per primitive. Point 132 primitives at one
2.1-million-vertex accessor and Bevy allocates 132 copies of all 2.1 million
vertices — 8.8 GB for a scene whose triangles fit in 140 MB, plus the tangents
it generates and the render world's copy, and the import dies on
`Caught rendering error: Out of Memory`.

So `glb_build::compact_groups` rebuilds the vertex buffer per group: each
primitive gets a private, contiguous slice covering only the vertices its own
triangles reference, and its indices are renumbered into that slice. Vertices
are duplicated only where a material seam genuinely needs it — on the exterior
above, 2,080,909 vertices for a 2,077,661-vertex source, or 1.00×. Bevy's
allocation drops from 8.81 GB to 0.10 GB.

> A useful sanity check on any GLB the importer writes: no accessor should be
> referenced by more than one primitive. If one is, the memory cost is
> multiplied by however many primitives share it.

Together these are what a missing material looks like end-to-end: with no
textures resolved, every `.material` a model produces has empty texture slots,
its graph is a bare Surface Output node with nothing wired into it, and the model
renders flat white.

**One file comes out per texture: the `.rmip`.** The GLB's own images point at
it directly, and so do the extracted materials.

This used to write a second copy in the source's own format, on the belief that
the GLB needed a format "Bevy's own image loader reads" in order to resolve. It
does not: `RmipAssetLoader` declares `Settings = ImageLoaderSettings` precisely
so Bevy's GLB loader can route a `.rmip` URI through it.

Keeping the companion was actively harmful. It doubled the texture footprint
exactly — 231 MB of pure duplication on a scene like Bistro — and it made the
GLB resolve through Bevy's **DDS** loader, which has no mapping for `ATI2`, the
FourCC every DCC tool writes tangent-space normal maps as. Those images failed
to load, and the model rendered untextured.

### DDS is repacked, not copied

An external `.dds` is turned into a `.rmip` rather than copied through.
Both hold GPU block-compressed mip chains, so this is a **repack**: the BC
blocks are copied across untouched and clamping is done by dropping whole mip
levels off the front. No decode, no re-encode — running a texture through RGBA
and back re-quantizes every block, and re-encoding a gigabyte of 2K maps to BC7
takes minutes for a worse result.

`DXT1`, `DXT5`, `ATI1`/`BC4U`, `ATI2`/`BC5U` and the `DX10` header's BC1/BC3/
BC4/BC5/BC7 codes all map onto `RmipFormat`. `ATI2` matters most: it's how
essentially every DCC tool writes tangent-space normal maps, and it's one of the
formats the `image` crate cannot decode — a decode-based pipeline would skip
exactly the maps a scene has most of. Anything unsupported (BC2, cubemaps,
volume textures) falls back to a verbatim copy.

The repack is what puts these textures under the engine's controls at all. A raw
`.dds` in a project is outside every one of them: nothing clamps its resolution,
and `renzora_engine::texture_stream` can only drop a material to a lower tier
when its textures are `.rmip`, since that's the format whose loader publishes the
`#low` mip-tail subasset.

### The texture budget

`ImportSettings::texture_max_size` caps a *single* texture, which is no
protection against a scene that stays under the cap several hundred times over.
A street exterior with 337 separate 2048² maps totals ~970 MB with every one of
them inside a 2048 cap, and in the editor's edit mode the whole set is resident
at once — the distance tier swap only runs while world streaming is active. The
result is `Caught rendering error: Out of Memory`, followed by a cascade of
invalid buffers as every allocation after it fails too.

So the importer totals the set up front — a DDS header states exactly how many
bytes the repack will produce at any cap, so this is arithmetic, not a guess —
and halves the cap until it fits **512 MB**, stopping at a 256px floor. Halving
the cap quarters the data, so it converges in a step or two: the exterior above
imports at 1024px and 243 MB. When the cap moves you get an import warning
saying so. A set already under budget is untouched, so ordinary props and
characters keep full resolution.

Only externally-referenced DDS is measured — its header gives an exact size for
free. An embedded PNG set big enough to matter would have made the source file
unopenable long before it reached the importer.

### The import overlay (`renzora_import_ui`)

The importer accepts more than 3D models. Every file falls into one of two
buckets, decided by `renzora_import_ui::kinds::detect_kind`:

- **Models** (glTF/GLB/FBX/OBJ/STL/PLY/USD/ABC/DAE/BVH/Blend) — run through the
  full GLB conversion pipeline with the model-only options below.
- **Copyable assets** — images (`png/jpg/jpeg/bmp/tga/webp/hdr/exr/ktx2/dds`),
  audio (`wav/ogg/mp3/flac`), `.bsn` scenes, `.particle`, `.material`, fonts
  (`ttf/otf`) and scripts (`lua/rhai`). These have no conversion step; importing
  one **copies it verbatim** into the destination folder (name-collisions get a
  numeric suffix, `tex.png` → `tex1.png`).

**Workflow.** Clicking the asset browser's **Import** button (or **☰ → File →
Import Assets…** in the top bar, or the command palette's *File: Import…*)
opens the **OS file picker first**, filtered to every importable kind. Once
files are chosen, the modal appears pre-loaded with them. Cancelling an empty
picker leaves the modal closed.

OS dialogs can't pick files and folders in one shot. To import a whole directory,
use **Browse folder** on the Files pane or **drag a folder** onto the asset
browser — both expand through the same detector, **preserve the source folder
tree** under the destination (including the selected folder's name), and land
in the batch queue. Drag-and-drop of individual files still works too (flat into
the target).

A folder holding nothing the importer recognises reports *"No importable files
in `<name>`"* in the modal rather than doing nothing. Symlinked subdirectories
are followed, but only once each — a link pointing back at an ancestor is
detected instead of walked forever. The walk runs on the main thread, so
dropping a very large tree stalls the editor until it finishes.

It's a **two-pane dialog**: a left sidebar lists the sections and a right pane
shows the active one, so the modal stays a fixed size instead of scrolling
through every option at once. The modal always opens on **Files**, and its title
tracks the queue's kind (*Import 3D Models* / *Import Images* / … / the generic
*Import Assets* for an empty or mixed queue).

- **Files** — a drag-and-drop card (**Browse files** / **Browse folder**) above
  the queued-file list. Each row's icon reflects the file's kind. The sidebar's
  Files row carries a count badge of how many files are queued.
- **Settings** — scale, up-axis, hierarchy, **Flip UVs** and **Generate
  normals** as label-left / control-right rows, fed into `ImportSettings`. A
  **Textures** dropdown appears too when the queue holds a geometry-only model
  with a `textures/` folder beside it — see
  [Sibling texture sets](#sibling-texture-sets). *Model-only: the nav row hides
  when the queue has no model.*
- **Extract** — toggles for skeleton/skin, animations, textures and materials.
  *Model-only.*
- **Optimize** — the mesh-optimization passes (vertex cache / overdraw / vertex
  fetch). *Model-only.*
- **Destination** — a **folder tree of the project's own directories** (the same
  picker style as the marketplace install flow). Click a folder to set the
  import target; the first row, *assets (project root)*, targets the project
  root. The **Organize** radios choose a per-file `<stem>/` folder or a combined
  destination (copied assets always land directly in the target folder).
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

**Where a drop lands.** While an OS file drag hovers the window, the asset
browser draws a **drop-to-import highlight** over its panel. The browser
republishes its current folder into `renzora::core::AssetBrowserCwd`, and the
importer's drop handler targets that folder — so a dropped file lands in the
folder you're looking at (and appears there once the browser's ~0.5 s rescan
picks it up), not the importer's default target. The hover flag itself lives in
`renzora::core::FileDragHovering`, set by the importer and read by the browser.

**Feedback on a drop.** A silent auto-import (no modal, no toast) still reports
itself two ways: the shell **status bar** shows a left-aligned `Importing
[done/total] …` item (registered by `renzora_import_ui` via the status registry,
live-updating each frame and blank when idle), and the browser **scrolls its grid
to the bottom** so the freshly-copied file scrolls into view. The scroll is
requested through `renzora::core::AssetDropScrollRequest` and held for a short
window so it tracks the grid growing as the rescan lands the new tile.

### The import window

Importing a model *is* inspecting it. Choosing a file opens a window — centred,
90% of the screen on each axis, over a dimmed editor — and conversion starts
immediately: there is no separate "import now" step and no toggle to turn
inspection on.

```
┌ Import — 1 of 3 ready  [BistroExterior.fbx ▾]   [1/3] …   ⊘Discard  ⏭Skip  ✓Add ┐
│ Files │ Scene │ Meshes │ Materials │ Destination                                │
├──────────────────┬──────────────────────────────┬────────────────────────────────┤
│ ☑ ▾ Bistro      ⇔│                             ⇔│ PROPERTIES                     │
│   ☑ ▾ Body       │        orbit · pan · zoom    │ surface  0                     │
│     ☑ ▾ BodyMesh │                              │ material Paint                 │
│       ☑   Paint  │                              │ FINDINGS — 2                   │
│       ☐   Chrome │                              │ IMPORT / EXTRACT / OPTIMIZE    │
└──────────────────┴──────────────────────────────┴────────────────────────────────┘
```

Actions and progress sit in the title bar rather than a footer: the decision
belongs next to what it is about, and a full-height window has no natural bottom
edge to anchor a bar to. The two column dividers drag to resize, and the widths
persist across opening and closing the window.

**One button writes anything, and it is Add to project.** Everything before it
happens in the project's cache directory. There is deliberately no *Import*
button: it named the step that started the conversion, which the window does on
its own, and it read as the step that adds the model — which is the one thing it
did not do.

**The dropdown beside the title switches between staged models** once more than
one is ready. A batch import stages every file and waits, so the window is
always showing one of several; changing which used to mean going back to the
Files tab and losing whichever tab you were working in.

| Tab | Holds |
|---|---|
| Files | Every staged model — click one to switch to it — above anything still queued |
| Scene | The hierarchy: node → its mesh → the mesh's surfaces, each with an include checkbox |
| Meshes | Flat mesh list with primitive and triangle counts; marks what the tree has excluded |
| Materials | Flat material list; selecting one renders it in the main viewport |
| Destination | Project folder tree and the per-file / combined layout choice |

The centre viewport is the staged model — **left-drag orbits, right or
middle-drag pans, wheel zooms** — or the selected material on a sphere. Camera
motion eases toward its target with a frame-rate independent exponential, and
zoom steps multiplicatively so each wheel notch is the same proportional move
whether you are close in or far out.

**The Scene tab mirrors the source hierarchy**, with a node's mesh hanging under
it and the mesh's surfaces under that — the mesh is a resource the node points
at, not the node itself, and collapsing them into one row hides which nodes
share geometry. Selecting a surface also points the Materials tab at its
material, so the two views agree about what you are looking at.

The tree is flattened to its visible rows and rebuilt through a keyed list
rather than nested as widgets: a scene can carry well over a thousand nodes, and
nesting a thousand collapsible widgets to show twenty of them is what makes an
ember panel drop frames. Rows are capped at 500 per rebuild.

**Every tree row has a checkbox, and unticking one leaves it out of the
import.** It works on all three levels — a node, the mesh under it, or a single
surface of that mesh.

- Unticking a node takes its **whole subtree** with it. The rows below go muted
  and stop responding: they are coming out either way, so their own boxes have
  nothing left to say.
- Ticking a node back **re-ticks everything under it**, including children that
  had been unticked individually. One click undoes a branch.
- What goes with the geometry goes too. A mesh nothing points at is dropped, a
  material no surviving surface uses is dropped along with its `.material` file,
  and a texture only that material read is deleted from the staged tree. The
  Meshes and Materials tabs mark those rows *not imported* as you go, so the
  consequences are visible before you commit.

Nothing is applied while you are looking at the model — the preview keeps
showing what the conversion produced, and unticking stays reversible. The edit
happens to the staging directory at the moment you press **Add to project**, so
what lands in the project is already the model you asked for
(`renzora_import::prune_glb`, then `compact_glb` to reclaim the orphaned
vertex data).

A skeleton is not a containment hierarchy, so joints of a surviving skin are
kept even when the branch they sit in is unticked — with any mesh they carried
stripped. Without that the `joints` array would name nodes that no longer exist
and the file would not load.

Reconverting rebuilds the model from scratch, and the indices the checkboxes
address are the converted GLB's own — so changing a setting clears them.

**Selecting a material renders it** on a UV sphere. That preview is assembled as
a `StandardMaterial` from the extracted PBR factors plus the staged `.rmip`
textures, *not* from a `.material` graph — the graph files are written on commit,
so during inspection they do not exist yet.

**Changing a setting reconverts on its own.** Conversion begins as soon as files
are chosen, so the settings in the rail describe a model that has already been
built; the window rebuilds it rather than leaving them as controls that do
nothing until a button is pressed. The reconvert waits ~0.9 s for the settings
to stop changing (a scrubbed drag field is one edit, not forty), discards
everything staged, and waits for the running worker to stop before starting
again — two workers must never stage into the same directories at once.

The destination counts as a setting here. The worker bakes the final paths into
each staged import and into the `.material` writes it is holding, so pointing
the window at another folder has to rebuild them too.

### Staging, and why accepting is instant

Every model is written to `<project>/.cache/import_staging/<n>/`. The worker
stages **all** of them back to back and then exits — it does not wait for a
verdict — so by the time you have finished looking at the first file the rest
are usually ready too, and the title-bar dropdown flips between them.

Files added to a window that already has models staged **join** them rather than
replacing them, so a second drop mid-inspection does not throw away what you
were looking at. Each run is handed its own block of staging slot numbers,
because the worker clears the directory it is about to write and reusing a
number would delete a tree the user still has on screen.

Staging is what lets the preview show a *textured* model: a GLB names its
textures by relative URI and Bevy resolves those against the file's own folder,
so the GLB and its `textures/` have to sit together. It is also why accepting is
fast — the tree is already complete and on the same volume as its destination,
so **Add to project** is a rename, not a copy of (for a large scene) several
hundred megabytes.

Verdicts are the window's to act on, not the worker's:

- **Add to project** — anything unticked in the scene tree is pruned out of the
  staged tree, then it is moved into its destination, the `.material` files are
  written, and a thumbnail capture is requested. The `PbrMaterialExtracted`
  events ride along inside the staged import and are held until this point,
  because the observer that handles them writes a file the moment it fires —
  and the ones for pruned-away materials are dropped rather than fired.
- **Skip** — the tree is deleted; the window moves to the next staged file.
- **Discard all** — every staged tree is deleted.

Closing the window discards whatever is left, so nothing lingers in the cache
with nothing referencing it. `.cache/` is not scanned by the asset browser, so a
staged import is invisible until it is accepted.

**Findings.** The right rail states what looks wrong, derived from the
pipeline's own output rather than a separate analysis:

| Finding | Raised when |
|---|---|
| Scene hierarchy flattened | one node holds many primitives, so nothing can be selected or culled individually afterwards |
| Primitives with no UVs | `TEXCOORD_0` is missing from some primitives, so textured materials render flat on them |
| Materials look alpha-tested | a material's name says foliage, glass, `.DoubleSided` or `_MASKED` but it imported opaque and single-sided |
| No material references a texture | every extracted material came out untextured |
| No animation clips extracted | an FBX or USD produced none |

Findings only report. Nothing in the findings list changes how a file was
converted — to change that, adjust the settings and let the window reconvert.

The public surface (`renzora_import`) includes `detect_format`, `supported_extensions`, `ModelFormat`, `convert_to_glb` / `convert_to_glb_with_progress`, `ImportSettings`, `UpAxis`, `optimize_glb`, `compact_glb`, `prune_glb` / `PruneSpec` (drop nodes, meshes or surfaces from a converted GLB and collect what that orphans), and `inspect_glb` / `GlbStats` (a structural summary of a converted GLB, read from its JSON without loading it), plus the `extract_animations_from_*` helpers.

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

`.ogg`, `.mp3`, `.wav`, `.flac` — decoded by the **audio backend plugin**, not by the engine: the engine reads the bytes (through the `.rpak` loader in an export) and hands them over. The bundled backend enables `ogg` and `wav` by default, with `mp3` and `flac` as cargo features, so a project carries only the decoders it uses. With no backend present nothing decodes and nothing errors. (The registry also tags `.opus` as audio, but no backend enables it yet.)

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
