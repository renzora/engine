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

**Every GLB leaving the pipeline is made loadable by a core-glTF consumer**
(`renzora_import::glb_compat`), which matters most for third-party downloads:

- `extensionsRequired` entries nothing here implements are dropped. The `gltf`
  crate — used by both `optimize_glb` and Bevy's loader — refuses to parse a
  document that lists an unknown extension as required, and the spec mandates a
  metal-rough fallback anyway.
- **`KHR_materials_pbrSpecularGlossiness` is baked down** into
  `pbrMetallicRoughness`: `diffuseTexture`/`diffuseFactor` become base colour,
  `glossinessFactor` becomes `1 - g` of roughness, and metallic goes to 0. Only
  materials whose metal-rough block says nothing beyond the glTF defaults are
  touched — a file carrying a real fallback keeps it.

  Dropping the *requirement* without this was half a fix. Plenty of spec-gloss
  exports carry no metal-rough data at all: every texture and colour lives
  inside the extension and `pbrMetallicRoughness` is either absent or written
  out at its defaults. Bevy ignores the extension, so the model loaded as
  untextured **white** — in the import preview and in the project alike. The
  per-pixel glossiness in `specularGlossinessTexture`'s alpha is not converted
  here (that needs a channel repack); material extraction already routes it into
  the `.material` graph's roughness pin.

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
files are chosen, what happens next depends on what was chosen:

- **A queue holding a model** opens the import window (below), because there is
  something to inspect: a scene tree, a preview, per-mesh include boxes.
- **A queue of copyable assets only** — textures, sounds, fonts, scripts,
  `.bsn`, `.particle`, `.material` — takes the **silent path** with the corner
  toast, exactly as dragging the same files in does. Importing one of these is a
  file copy; a modal asking you to confirm it, with three tabs that had nothing
  to show and a preview that stayed empty, was the window doing the wrong thing.

Cancelling an empty picker leaves the window closed.

OS dialogs can't pick files and folders in one shot. To import a whole directory,
use **Add folder** on the Files tab or **drag a folder** onto the asset
browser — both expand through the same detector, **preserve the source folder
tree** under the destination (including the selected folder's name), and land
in the batch queue. Drag-and-drop of individual files still works too (flat into
the target).

A folder holding nothing the importer recognises reports *"No importable files
in `<name>`"* in the modal rather than doing nothing. Symlinked subdirectories
are followed, but only once each — a link pointing back at an ancestor is
detected instead of walked forever. The walk runs on the main thread, so
dropping a very large tree stalls the editor until it finishes.

The window's title tracks the queue's kind (*Import 3D Models* / … / the generic
*Import Assets* for an empty or mixed queue); its layout is described under
[The import window](#the-import-window).

The silent path hands progress to a **corner toast** (bottom-right): a live
`[done/total]` label + progress bar while the background worker runs, then a
success/error line that auto-dismisses after a few seconds (or via its ×
button). The import keeps running in the background regardless of whether the
toast is dismissed. Finishing an inspected import hands off to the same toast,
so closing the window is not the same as the import going unremarked.

> Drag-and-drop with **Auto-import on drop** enabled (the default) skips the
> window entirely and imports silently; a model dropped with it *disabled* opens
> the window to be confirmed. Copyable assets never open it either way.

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
┌ ⬡ Import        Files  Scene  Meshes  Materials  Destination         [✓ Import]  ✕ ┐
├──────────────────┬──────────────────────────────┬────────────────────────────────┤
│ ☑ ▾ Bistro      ⇔│                    ⊕Y      ⇔│ PROPERTIES                     │
│   ☑ ▾ Body       │                    ╲Z        │ surface  0                     │
│     ☑ ▾ BodyMesh │                 ⊕──⊕X        │ FINDINGS — 2                   │
│       ☑   Paint  │                 [+][-][⊹]    │ IMPORT / EXTRACT / OPTIMIZE    │
│       ☐   Chrome │        orbit · pan · zoom    ├────────────────────────────────┤
│                  │        PREVIEW ────────────┐ │        [ ✓ Import ]            │
└──────────────────┴──────────────────────────────┴────────────────────────────────┘
```

**Everything lives in one header row**: the title on the left, the tabs centred,
**Import** and **Close** on the right. It is the shape the editor's own top bar
already has — identity, then navigation, then actions — and collapsing the
separate tab strip into it gives the preview a whole row of height back.

The viewport's own overlays all live down its right edge: the gizmo, the zoom
cluster and the lighting card. Conversion progress is deliberately *not* among
them — the Files tab already reports it per file, on the row it belongs to, and a
second readout floating over the preview read as the preview still loading rather
than as a file still converting.

The header used to name the staged file too, with a count and a dropdown to
switch between them. Both are gone: the Files tab *is* that list, with the
selection, the per-file findings and a trash on each row, so the header was a
second and worse copy of it that pushed the tabs off centre by however long the
current file name happened to be. Conversion progress moved out as well, to the
bottom centre of the viewport — progress is about the thing being looked at, and
the centre is where you are already looking.

The two column dividers drag to resize — the drag tracks the **cursor's
position**, not accumulated mouse motion, so the handle stays under the pointer
however fast it moves — and the widths persist across opening and closing the
window.

**One button writes anything, and it is Import.** Everything before it happens in
the project's cache directory. It takes **every** staged file, not just the one
on show: one button per file made a batch a row of identical decisions the user
had already made by queueing them. What is left per-file is *rejection* — the
trash on each row of the Files tab — which is the choice that actually differs
between them.

The right rail has a footer only when a **Reconvert** is owed; it is absent
otherwise rather than an empty bordered band.

Once the last staged file has been dealt with and nothing is queued or
converting, **the window closes itself**. It used to sit there empty, which
reads as a dialog that has failed to notice it is finished. A run that reported
a failure keeps it open, because the Results list in the rail is the only place
that failure is written down.

**The dropdown beside the title switches between staged models** once more than
one is ready. A batch import stages every file and waits, so the window is
always showing one of several; changing which used to mean going back to the
Files tab and losing whichever tab you were working in.

| Tab | Holds |
|---|---|
| Files | Every staged model — click one to switch to it — above anything still queued. Each row carries a **trash** that throws that one file away |
| Scene | The hierarchy: node → its mesh → the mesh's surfaces, each with an include checkbox |
| Meshes | Flat mesh list — **names only**, and clicking one isolates that mesh in the preview. A scan gives forty rows of forty-character names in a 310px column and there is no width left for a count that reads the same on nearly all of them; both counts are in the properties rail for whichever mesh is selected |
| Materials | Flat material list; selecting one renders it in the main viewport. Names only — `Opaque · 2-sided` was the same two words on nearly every row, distinguishing nothing while overrunning the names that do, and both are in the properties rail for the selected material |
| Destination | The **layout** radios — a per-file `<stem>/` folder or a combined destination (copied assets always land directly in the target folder) — over the project's folder tree, with **New Folder** beneath it. The layout leads because it decides what the tree's answer *means*; New Folder trails because it acts on whatever the tree has selected |

**A queue row says where it is in the pipeline.** A file the worker has not
reached is drawn grey and hovers a *Waiting in queue* tooltip; the one it is on
carries a sweeping progress bar along its bottom edge and reports its phase
(*Converting …*, *Compressing textures: …*, *Optimizing …*) on its second line;
a finished one leaves the queue, because a converted model is already sitting in
the staged list above it and a copied file is already in the project. Showing
both made a batch look like it had queued everything twice.

The bar is a sweep rather than a fraction on purpose. The worker's
`[done/total]` counts *files* until texture baking starts and then counts
*textures*, and the phases after it (optimize, animation extraction, compaction)
report no number at all — so a determinate bar on one row would jump backwards
and then stall. Which file is going is the thing the row has to say; the numeric
progress is under the centre's placeholder, and in the status bar.

Which file that is comes from a `FileStarted` message the worker sends as it
reaches each one, not from the progress counters, for the same reason: the
texture callback overwrites `current`/`total`, so the file index in them is not
even monotonic.

Clicking a file in the Files tab **stays on Files**. It used to jump to Scene,
which threw you out of the list you were working down every time you picked the
next file out of it; the row's highlight and the preview changing are the
feedback that the click landed. Nothing switches the tab on its own any more —
not a file finishing conversion either.


The overlays serve **both** previews. The material sphere is lit by the same
environment and the same rig as the model, so the switches mean the same thing on
either — and a material is judged by what it reflects, which is where the
Environment switch matters most. It has an orientation and a distance too: it is
a UV sphere precisely so its seam and poles show a wrongly-tiled texture, and
telling which way round it is turned is exactly what the gizmo is for.

**An axis gizmo sits in the viewport's top-right**, where the editor viewport
keeps its own — they are the same control and belong in the same place, so the
lighting panel gave the corner up and moved to the bottom. It is drawn larger
than the editor's: this one is the only way to reorient a camera with no
keyboard shortcuts, so its tips are click targets rather than an orientation
readout you glance at.

The **zoom buttons** are pinned to the middle of the right edge rather than hung
under it. They are not part of the gizmo — one says which way the model is
facing, the other how close it is — and stacked directly beneath it they read as
its tail, a lopsided column growing out of one corner. Click a tip to swing to that view, drag the backplate to
orbit, and the three buttons zoom in, out and back to the framing distance. The
zoom step and its bounds are the wheel's, so a press and a notch are
interchangeable and neither can leave the model behind — the bounds are relative
to the framed distance, since an absolute limit means something different for a
20 cm prop and a 200 m street.

It is a separate implementation from the editor viewport's gizmo, not a reuse.
That one is wired through the viewport slot system — `Viewports.slots[i]`,
`CameraOrbitSnapshot`, `NavOverlayState`'s atomics,
`ViewportSettings::pending_view_angle` — none of which an import preview has or
should acquire; reusing it would mean depending on `renzora_viewport` and
inventing a fake viewport slot for a camera that is not one. What *is* shared is
the part worth sharing: the same projection maths and the same axis colours, so
a given orbit angle draws the identical cluster in both and the two need no
separate muscle memory.

The gizmo is drawn from the camera's **smoothed** angle, not its target. The
camera eases toward a new view over several frames, and a gizmo drawn from the
target arrives at a snapped view before the model it is describing does.

The model is framed from the true world AABB of its meshes: the eight corners of
each mesh's box, transformed. Growing the bound by each mesh's *sphere* radius on
every axis (the cheaper thing it used to do) only over-pads, but the grid is
placed at the bound's minimum Y — and a sphere radius below a wide flat model is
a long way below its feet, so the model hung in the air over its own floor.

The camera then fits that **box as the default view sees it**, against the render
target's real aspect, rather than fitting a bounding sphere. For an axis-aligned
box the projected half-extent along a screen axis is the dot of the half-extents
with that axis's absolute components — exact and cheap — so the distance is
whichever of the two axes binds. A sphere fit is what a turntable needs, since it
cannot clip a corner into frame whatever the spin, but nothing here spins and for
the shapes that actually get imported it wastes most of the frame: a car is long,
low and thin, so its bounding sphere is close to its *length* and fitting that to
the frame left the car occupying about a third of the width. Fitting the box is
also why the fill fraction can sit at 0.95 rather than the ~0.8 a sphere fit
needed to avoid clipping.

The centre viewport is the staged model — **left-drag orbits, right or
middle-drag pans, wheel zooms** — or the selected material on a sphere. Camera
motion eases toward its target with a frame-rate independent exponential, and
zoom steps multiplicatively so each wheel notch is the same proportional move
whether you are close in or far out.

**How the preview is lit is yours to choose**, from a small **PREVIEW** panel
floating over the viewport's top-right corner. It sits there rather than in the
settings rail because it changes nothing about the import — the rail is what the
file will *become*, and mixing a view setting into it would make every row there
suspect.

| Toggle | What it does | Default |
|---|---|---|
| Environment | The studio cubemap: image-based lighting, the sky it is drawn from, and the backdrop behind it, as one switch. This is what stops the shadow side reading as crushed black, and the only thing that gives a glossy surface something to reflect — a car's paint does not read as paint without it | on |
| Lights | The three-point directional rig, and with it the only shadows. Off, you see the model under the environment alone | on |
| Grid | The floor under the model, on the preview's own render layer and **scaled to the model** — roughly eight squares across it, sitting at the model's feet, so a coin and a warehouse both get a floor that reads. The editor's own grid is sized to the viewport camera's height and could never do that | on |

Lighting, sky and backdrop are **one** toggle, not three, and each split was
wrong for its own reason. The lighting half changed a surface subtly enough that
against the key light it read as doing nothing at all. A visible sky with no
reflections — or reflections with no visible sky — is not a state anyone wants.
And the backdrop colour was only ever seen with the sky *off*, so it was a
control that appeared to do nothing until another one was changed first. An
environment is what you see and what lights you.

The cubemap is **built in code, not shipped**: six 64px faces of a vertical
gradient plus a soft hot spot where the key light points, so the environment and
the rig agree about where the light is coming from. It costs one 192 KB image
and no asset file, which matters because the alternative is an HDRI in the repo
that every export template would carry.

It is **two bands with a horizon between them**, matched to the editor
viewport's own sky — pale haze at the horizon climbing to a light desaturated
blue, over flat warm tan. Sky and floor each have their own pair of endpoints
and never meet in the middle; only a two-degree smoothstep joins them, wide
enough that a 64px face does not stair-step and narrow enough to read as a line.

The viewport's sky is Bevy's procedural `Atmosphere`, and the palette here is
*matched* to it rather than shared, deliberately: that sky follows the project's
World Environment, so borrowing it would make a preview look different depending
on what time of day the user's scene is set to, and dark at midnight.

That shape is the fix for a failure every earlier version shared, and it is not
an obvious one. They all lerped *outward from* a common horizon colour, so
everything near the horizon was that one colour — and a camera framed on a model
looks very slightly down, so it sees almost nothing **but** the horizon. Whatever
that shared colour was became the entire backdrop, and the interesting colours at
the two extremes sat off-screen. With a neutral grey in the middle the result was
a flat grey wash; the blue and the cream were both there and both invisible.

Two other things had to be right for the colour to survive at all:

- **The tonemapper desaturates what it compresses.** A bright saturated blue at
  1500 cd/m² came back out as pale haze that read as fog. Brightness is not what
  makes a sky look like a sky. It is drawn at **1000**, which is chosen so the
  palette is readable rather than arbitrary: Bevy's default camera exposure is
  `Exposure::BLENDER` (EV100 9.7), whose scale is `1 / (2^9.7 * 1.2)` ≈ 1/998,
  so each value lands on screen as very nearly the colour written. It also keeps
  every channel under 1.0, which is the part that matters — that is the knee the
  earlier version was pushed past.
- The lower half is **cream, not a dark floor**, because it is two things at
  once: the ground the model stands on, and the fill light on everything facing
  down. Dark there meant a sky over a pit and undersides that went to nothing.

Switching the environment off takes the backdrop with it: the camera clears to
near-black, leaving the model and the grid and nothing else. A bright backdrop
left behind read as the sky having been swapped for a flat wall rather than
removed.

The one switch drives two consumers, in **opposite** ways, and the difference is
not arbitrary:

- `GeneratedEnvironmentMapLight` is attached to the camera **at spawn** and only
  ever changed by its `intensity`. A camera's bind group layout is fixed on its
  first rendered frame with the IBL slots present only if the component was
  there, so attaching one later is a wgpu validation failure rather than a
  relight — the constraint `renzora_environment_map` documents.

  The intensity has to be written to the **derived** `EnvironmentMapLight` as
  well, and that is what makes the toggle do anything at all.
  `generate_environment_map_light` runs once, on a `Without<EnvironmentMapLight>`
  query: it copies the intensity into the component it inserts and then never
  looks again, because the entity now has the component that filtered it out.
  Writing only the source left that copy frozen at the camera's first frame.
- `Skybox` is **added and removed outright**. It is a standalone render pass
  rather than a mesh-view binding, so it is not subject to that lock (the
  engine's own shared-sky fan-out in `renzora_engine::camera` relies on the same
  thing), and toggling it by `brightness` would not work anyway: brightness zero
  paints black, swallowing the backdrop colour the other toggle exists to pick.

Six array layers is not by itself a cubemap. The IBL path does not care, but
`Skybox` checks the *view* dimension and silently skips an image whose view is
the default 2D-array, so the image sets `TextureViewDimension::Cube` explicitly.
The only sign of getting that wrong is a `warn_once!` and a backdrop that never
appears.

**The Scene tab mirrors the source hierarchy**, with a node's mesh hanging under
it and the mesh's surfaces under that — the mesh is a resource the node points
at, not the node itself, and collapsing them into one row hides which nodes
share geometry. Selecting a surface also points the Materials tab at its
material, so the two views agree about what you are looking at.

**Selecting isolates.** Everything outside the chosen subtree is hidden and the
camera reframes on it, so a single lamp post in a street scene is actually
visible rather than a speck the camera has flown to. It works from the Meshes tab
as well as the scene tree — the same mesh answering the same question — and
clearing the selection restores the whole model.

**Row labels are elided in the middle**, not clipped on the right. Imported names
are long and differ at the *tail* — a scanned building gives forty meshes called
`TexturesCom_WindowsBacklit0019_13_M_0`, where every distinguishing character is
in the last eight — so a right-clip throws away exactly the part worth reading.
The string is shortened rather than the node clipped because bevy_ui has no
text-overflow ellipsis, which leaves a hard cut with no sign anything was
removed, or what this used to do: wrap, out of a fixed 22px row and over the row
beneath it.

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
happens to the staging directory at the moment you press **Import**, so
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

**Changing a setting offers a Reconvert.** Conversion begins as soon as files
are chosen, so the settings in the rail describe a model that has already been
built. When they stop matching it, a *Settings changed since this was converted*
note and a **Reconvert** button appear at the bottom of the settings group.

This used to happen on its own, on a ~0.9 s settle timer, and that read well for
the scale field and badly for everything else: ticking *Overdraw* in the
Optimize group threw away a converted 900k-triangle scene and rebuilt it for
half a minute, over an option that changes what is **written** rather than what
is on screen — and flipping four of them in a row meant four rebuilds, each one
resetting the preview camera.

The note and its button are the rail's **footer**, not the bottom of the settings
scroll. That was the one place a notice cannot do its
job: the moment it appeared it was below the fold, so the model sat there stale
with nothing on screen saying so.

Reconverting discards everything staged and waits for the running worker to stop
before starting again: two workers must never stage into the same directories at
once. The indices the scene tree's checkboxes address are the converted GLB's
own, so a reconvert clears them.

The destination counts as a setting here. The worker bakes the final paths into
each staged import and into the `.material` writes it is holding, so pointing
the window at another folder has to rebuild them too.

**Boolean settings are switches, not checkboxes.** The rail's options are
*options*; the window's one real checkbox is the include-box in the scene tree,
which means something else entirely. Two controls that looked alike and meant
different things was the confusion worth removing.

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
so **Import** is a rename, not a copy of (for a large scene) several hundred
megabytes.

While a staged model is loading into the preview, the centre shows an
indeterminate **Loading preview…** bar on a card — it floats over whatever the
camera is already rendering, which is the previous model's last frame or the
clear colour, and bare text on that is at the mercy of what is behind it. While
files are still converting the centre shows the worker's `[done/total]` as a
determinate bar instead. A staged scene is often
hundreds of megabytes and Bevy's loader reports no fraction, so between staging
finishing and the model appearing the centre was otherwise a flat empty
rectangle for several seconds, which read as a preview that had failed.

Verdicts are the window's to act on, not the worker's:

- **Import** — for every staged file: anything unticked in the scene tree is
  pruned out of the staged tree, then it is moved into its destination, the
  `.material` files are written, and a thumbnail capture is requested. The
  `PbrMaterialExtracted` events ride along inside the staged import and are held
  until this point, because the observer that handles them writes a file the
  moment it fires — and the ones for pruned-away materials are dropped rather
  than fired.
- **The trash on a row** — that one tree is deleted; the rest are untouched.
- **Close** (the ✕ in the title bar) — every staged tree is deleted.

Closing the window discards whatever is left, so nothing lingers in the cache
with nothing referencing it. `.cache/` is not scanned by the asset browser, so a
staged import is invisible until it is accepted.

**Opening a project clears the staging directory outright.** Closing the window
is the *tidy* exit; quitting the editor with an import still open, a crash, or a
kill leaves the whole converted tree — GLB, extracted textures and all — with
nothing referencing it and nothing that would ever look at it again. It is not
small: one abandoned session of four scanned models measured **775 MB**, and
every later session added its own on top, because slot numbering restarts at
zero per process and only overwrites the slots it actually reuses. Project open
is the one moment nothing can be staged yet, which is what makes a blanket
delete safe there and unsafe anywhere else.

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
| `.lua` | Lua (mlua, Lua 5.4) — from `plugins/lua`, so **native only**: the web build cannot load a backend plugin |
| `.rs` | Rust scripts — compiled to a native plugin per script, native only |
| `.rhai` / `.js` / `.ts` | Tagged `AssetKind::Script` with a code icon, but **there is no backend** for any of them — cosmetic recognition only. (Rhai's backend was removed.) |

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
