# Terrain

Sculpt and paint heightmap terrain in the editor with a live brush gizmo, then save it straight into your RON scene.

## How it works

Terrain is two crates working together:

- **`renzora_terrain`** — the runtime. `TerrainPlugin` registers the data types, builds chunk meshes, composes heightmaps, uploads splatmaps, and scatters foliage. It self-registers with `renzora::add!(TerrainPlugin)`, so terrain renders in **both the editor and your shipped game**.
- **`renzora_terrain_editor`** — the editor-only tools (`TerrainEditorPlugin`, `Editor` scope): the brush gizmo, sculpt/paint systems, the **Terrain Tools** panel, undo/redo, and heightmap import/export. Foliage painting is a separate editor crate, `renzora_foliage_editor`.

A terrain is a **parent entity** (`TerrainData`) with one **chunk child** (`TerrainChunkData`) per tile. Each chunk stores a square grid of heights normalized to `[0, 1]`; the chunk's `TerrainData` maps that range onto world `min_height..max_height`. Sculpting writes the chunk's `base_heights`; a composition pass adds any per-layer carve deltas to produce the final `heights` the mesh and collider read.

> There is **no terrain scripting API**. Older docs showed `terrain_get_height(x, z)` / `terrain_set_height(x, z, h)` and globals like `position_x` — none of these exist in Lua or Rhai. Terrain is authored in the editor and serialized into the scene; runtime height queries are done with a standard mesh raycast.

## Creating terrain

Add a terrain from the editor's **Add** menu (it's registered as the `terrain` entity preset, icon "mountains", category "general"), or click any terrain toolbar button while no terrain exists.

A fresh terrain is a **single 64 m × 64 m tile** at `chunk_resolution = 129` (129 × 129 vertices), with a checkerboard placeholder material and a flat surface sitting just above the editor grid plane. The default height range is `-10 m` to `40 m`.

The parent entity spawns at `y = 0.05` rather than `y = 0`. A new terrain's flat surface would otherwise be exactly coplanar with the grid, and the grid — which draws in the transparent pass without writing depth — breaks up into shimmering patches against a surface at its own depth. The 5 cm lift puts the terrain strictly in front, so it occludes the grid cleanly. It's an ordinary Transform, saved with the scene: move the terrain to `y = 0` yourself if you want it exactly on the plane.

> Chunks each get a **trimesh collider** (`renzora_physics`'s `CollisionShapeData::mesh()`). Rebuilding a full-chunk trimesh is expensive, so the collider is **debounced**: it rebuilds ~0.25 s after the last mesh change and never mid-stroke — sculpting stays responsive, and the collider catches up on release. It is a triangle mesh, *not* a heightfield collider.

### The Terrain inspector component

Selecting the terrain root shows a **Terrain** section in the inspector: a read-only **Size** summary, an **Edit Terrain…** button that opens the [Terrain Settings overlay](#the-terrain-settings-overlay), and the live fields — **Min/Max Height** (clamped to keep ≥1 m of range) and the **Stream Chunks** / **Stream Radius** pair.

> The *structural* fields — grid size, chunk size and resolution — are deliberately **not** live inspector fields. A scrubbable field writes on every tick of the drag, and each write respawns every chunk with a fresh trimesh collider, so dragging the grid from 1 to 8 built every size in between. They live in the overlay instead, which stages the edit and applies it once. What's left in the inspector is the set that rebuilds in place, where a live drag is cheap.

A **Layers** section sits below it, editing the *active* paint layer: a layer picker, **Name**, **Material** (`.material` drop), **Height Offset**, **Coverage Threshold**, an **Enabled** toggle (hides that layer's overlay), plus **Add Layer** / **Remove Layer** buttons. It's the same data the Terrain Tools panel's layer list edits.

## The Terrain Settings overlay

**Edit Terrain…** opens a modal holding everything that changes the terrain's structure. Nothing is written until you press **Apply**, so the expensive rebuild happens exactly once, for the size you actually chose.

- **Grid** — click a cell in the picker to set the chunk grid; the clicked cell is the far corner, so the top-left cell is a 1×1 terrain. Sizes past 12 per side go through the **Chunks X** / **Chunks Z** fields beside it. The cap is 32 per axis.
- **Chunk Size** (8–512 m) and **Resolution** (33/65/129/257 — fixed steps, so neighbouring chunk edges share vertices exactly).
- **Min / Max Height**, **Stream Chunks**, **Stream Radius**.
- A live **cost readout**: chunk count, total vertices, estimated memory and collider count, with a warning band past ~2 M vertices. Every chunk also builds a triangle-mesh collider, which is the part that actually makes a big rebuild slow.

**Cancel** or **Escape** discards the draft and leaves the terrain untouched.

## Resizing with the Region tool

The **Resize Terrain** toolbar button turns on the Region tool, which grows and shrinks the terrain by clicking in the scene rather than by typing numbers.

- Ghost tiles ring the terrain, each marked with a `+`. **Click one** to grow the terrain to meet it.
- **Ctrl+click** an edge tile to remove that whole row or column; the row that would go is highlighted in red first.
- Only edge-adjacent ghosts are offered — there are no diagonal ghosts, since a rectangle can't grow "diagonally" without adding two rows at once.

The chunk grid is centred on its parent, so changing the count re-centres every chunk. The tool compensates for that: growing on the −X or −Z side re-indexes the surviving chunks and shifts the terrain's transform by half a chunk the other way, so **terrain you have already sculpted does not move**. That arithmetic lives in `renzora_terrain::grid` and is unit-tested on all four edges in both directions.

> The grid stays a dense rectangle. Sparse, non-contiguous regions would be a rewrite of `TerrainData`, the chunk addressing and the scene format.

## The tool shelf and the brush bar

With a terrain tool active, the brushes live on the **tool shelf** — a two-column palette floating down the viewport's left edge, in the shape image editors use. The sculpt palette shows all 17 sculpt brushes; switching to the paint tool swaps it for the 4 paint brushes, and switching to Paint Foliage swaps it for the [foliage palette](#the-foliage-shelf). The shelf collapses entirely when no shelf tool applies.

The active brush's settings sit in the **viewport toolbar** as a group you can drag to a different position on the bar: **Size**, **Strength** and **Falloff** sliders, the **shape** toggles (circle / square / diamond) and the five **falloff-curve** letters (S / L / O / T / F). Whatever the current brush adds appears beside them and nothing else does — Flatten's mode and target height, Noise's mode/scale/octaves/persistence, Terrace's steps and sharpness, Stamp's blend/rotation/scale.

## The Terrain Tools panel

The **Terrain Tools** panel (panel id `terrain_tools`, category "Terrain" in the dock's panel picker) is the secondary surface: it carries heightmap import/export, stamp loading and the layer list. It still has its own brush grids, and they stay in sync with the shelf for free — both write the same `TerrainSettings.brush_type`.

The panel and the viewport toolbar drive the same state, from either direction:

- Toggling **Enable Terrain Mode** on selects the first terrain and arms the current tab's tool; toggling it off returns to **Select**.
- Clicking the panel's **Sculpt**/**Paint** tab switches the active tool with it.
- Activating a terrain tool from the viewport toolbar reveals the panel body and switches its tab to match.

Four toolbar buttons appear in the viewport toolbar whenever a terrain exists in the scene (section "Terrain"):

| Button | Tool | Inspector tab |
|--------|------|---------------|
| **Sculpt Terrain** | `TerrainSculpt` | Sculpt |
| **Paint Terrain Layers** | `TerrainPaint` | Paint |
| **Paint Foliage** | `FoliagePaint` | — (Foliage panel) |
| **Resize Terrain** | `TerrainRegion` | Region |

Clicking a button selects the first terrain, switches the tab, and activates the tool. Click the active button again to return to the **Select** tool.

## Sculpting

Pick a brush from the shelf (or the panel's grid), then paint in the viewport. The brush position is found by **mesh raycast** against the actual sculpted surface, so the gizmo hugs the terrain. Hold the left mouse button and drag to sculpt continuously.

| Brush | Behaviour | Shift held |
|-------|-----------|------------|
| **Sculpt** | Raise terrain under the brush | Lower |
| **Raise** | Push up | — |
| **Lower** | Push down | — |
| **Smooth** | 3×3 weighted average toward neighbours | — |
| **Flatten** | Level toward the height where the stroke began (`Both` / `Raise` / `Lower` mode) | — |
| **Set H** | Ease toward the **Target Height** value | — |
| **Erase** | Reset toward the flat baseline | — |
| **Noise** | Add fractal noise (FBM / Ridge / Billow / Warped / Hybrid) | Box-smooth |
| **Terrace** | Snap heights to stepped plateaus (Steps / Sharpness) | — |
| **Ramp** | Gradient toward the stroke-start height across the brush | Flip direction |
| **Erosion** | Thermal erosion — lower vertices steeper than the talus angle | — |
| **Hydro** | Hydraulic erosion — sediment flows downhill | — |
| **Pinch** | Amplify deviation from the local average | Smooth toward average |
| **Relax** | Laplacian relaxation toward the 4-neighbour average | — |
| **Retop** | Wide 5×5 aggressive smooth | — |
| **Cliff** | Amplify the local slope gradient (steepen) | Soften |

The 17th tool is **Stamp** — click (don't drag) to stamp a heightmap shape once. Its settings offer a **Shape** preset (Dome, Cone, Bell, Mesa, Ridge, Crater, Noise), a **Load PNG…** button for a custom grayscale stamp, a **Blend** mode (Add / Subtract / Replace / Max / Min), **Rotation** (degrees), and **Height Scale**. Brush size sets the stamp footprint; picking Stamp with nothing loaded auto-selects the Dome preset.

### Brush settings

These live in the viewport toolbar's terrain group (and, in fuller form, in the Terrain Tools panel).

- **Size** — brush radius in **world metres** (`1`–`200`). The scroll wheel resizes it (×1.1 / ×0.9) while hovering the viewport.
- **Strength** (`0.01`–`1.0`).
- **Falloff** (`0`–`1`) — how far the soft edge reaches in from the rim.
- **Shape** — **Circle**, **Square**, or **Diamond**.
- **Falloff curve** — **S**mooth (cosine), **L**inear, Spherical (**O**), **T**ip, or **F**lat.

Per-brush additions, shown only for the brush that uses them:

- **Flatten** — **Mode** (Both / Raise / Lower) and **Target Height** (`0`–`1`).
- **Noise** — **Mode**, **Scale**, **Octaves**, **Persistence** on the toolbar; **Lacunarity**, **Seed** and (in Warped mode) **Warp** strength in the panel.
- **Terrace** — **Steps** and **Sharpness**.
- **Stamp** — **Blend**, **Rotation** and **Height Scale** on the toolbar; the preset picker and **Load PNG…** in the panel.

The gizmo draws an outer ring plus an inner falloff ring (and a vertex-density grid preview for the Stamp brush).

### Undo / redo

Sculpt and paint strokes are snapshotted on mouse-down and recorded onto the editor's central **Scene undo stack** on mouse-up, so they appear in the History panel alongside every other scene edit. **Ctrl+Z** undoes, **Ctrl+Y** (or **Ctrl+Shift+Z**) redoes.

## Heightmap import / export

The Sculpt tab's **Heightmap Import** section has **Import Heightmap…** and **Export Heightmap…** buttons.

- **Import** accepts **8- or 16-bit grayscale PNG** (8-bit RGB/RGBA use the red channel) or **RAW16** (`.r16` / `.raw`, 16-bit unsigned, row-major). The image is bilinearly resampled across every chunk and written into `base_heights`.
- **Export** writes a single **16-bit grayscale PNG** of the composed heightmap across all chunks.

## Painting layers

The **Paint Terrain Layers** tool paints coverage masks into the terrain's **`Painter`** component — a stack of paint layers on the terrain root entity. Pick a paint mode from the shelf (or the panel's grid):

| Tool | Effect |
|------|--------|
| **Paint** | Stamp the active layer's coverage (idempotent — overlapping strokes don't amplify) |
| **Erase** | Remove the active layer's coverage |
| **Smooth** | Blur the active layer's mask against itself |
| **Fill** | Set coverage to full under the brush |

### Layers

A `Painter` holds **up to 8 layers** (`MAX_LAYERS = 8`). A fresh terrain starts with **no layers** — click **Add Layer**, or just start painting: the first stroke auto-creates **Layer 1**.

Each layer is pure data: a coverage **mask** (one cell per terrain vertex), an optional **`.material`** path, a **height offset**, and an enabled flag. Painting is **non-destructive** — each layer keeps its own mask, and erasing or disabling a layer never touches the others.

Layers render as **overlay meshes**: where a layer's mask exceeds its coverage threshold, matching terrain triangles are emitted slightly above the surface (`height_offset`, default `0.02`), following the sculpted heights as you edit. The overlay meshes are derived data — hidden from the hierarchy panel and never saved; the masks on the `Painter` are what persists.

In the **Layers** section: click a row to select the active layer, use **Add Layer** (hidden once 8 layers exist), and drop a **`.material`** asset onto the active layer's drop zone to drive its appearance (albedo / normal / ARM texture paths are extracted from the material graph). The ✕ clears the assignment, reverting the layer to a neutral grey.

Paint strokes, including a stroke that auto-created a layer, undo/redo as single steps alongside sculpt strokes.

### Paint Brush Settings

- **Size** (`0.01`–`0.5`) — brush radius as a **fraction of a chunk side** (the scroll wheel resizes within that range).
- **Strength** (`0.01`–`1.0`), **Falloff** (`0`–`1`), and **Shape** (Circle / Square / Diamond).

## Foliage

Foliage is the separate **Paint Foliage** tool (`renzora_foliage_editor`, panel id `foliage_painting`) — not part of the Terrain Tools panel, which only links to it. You paint a per-chunk **density map** (`FoliageDensityMap`), and the runtime bakes animated grass blades into the painted areas, re-baking as you sculpt underneath. The density map serializes with the scene.

Grass appears **as you drag**. A rebuild rescatters the chunk's entire blade set, so the preview runs on a duty cycle paced by what that actually costs (`FoliageRebuildCost`, measured live): a bare chunk previews at 20 Hz, an expensive one backs off to as slow as 2 Hz, and every chunk the stroke touched is rebuilt once more on release so the result never ends on a stale preview. Since blades became GPU instances the scatter no longer builds geometry, so most strokes now sit at the fastest interval.

### How grass renders

Grass draws through its **own instanced render pipeline** (`renzora_terrain::foliage::render`), not through a Bevy `Material`. One draw call per chunk per foliage type, `24` vertices by however many blades that chunk scattered.

The blade's *shape* is rebuilt in the vertex shader from `@builtin(vertex_index)`, so it costs no memory at all; the only per-blade data that reaches the GPU is a 48-byte instance record — position, height, width, wind phase, bend, lean, and the sine/cosine of its rotation. The previous design baked every blade into one giant mesh per chunk at roughly **560 bytes a blade**, which is what forced a low blade budget, made repainting expensive, and put a ceiling on density. Instancing is a **~12×** reduction, and it is why the budget could go from 250 K blades to 2 M.

Blades are drawn **double-sided**. A back-face-culled blade simply vanishes when you walk behind it — about half of them at any moment — and the fragment shader flips the normal so the back face is still lit rather than reading as a black cut-out.

> **Grass does not cast shadows and does not write into the depth prepass.** The old baked mesh was an ordinary `Material`, so it landed in the shadow and prepass pipelines for free; a hand-written pipeline does not. Depth-driven effects (SSAO, contact shadows) currently see through grass. Restoring it means a second pipeline running the same vertex expansion against the `Shadow` and prepass phases. Grass is also absent from the viewport's debug visualization modes for the same reason — there is no material asset to swap.

### The foliage shelf

With **Paint Foliage** active, the same left-edge [tool shelf](#the-tool-shelf-and-the-brush-bar) the terrain brushes use swaps to the foliage palette, in two groups:

- **Paint / Erase** — the brush mode.
- **A numbered button per foliage type** — 1 to 8, matching the numbering in the panel's Foliage Types list. A button appears only once that type exists, and its tooltip shows the type's current name, so renaming a type in the panel renames it on the shelf.

Eight is the ceiling: a density map carries eight weights per texel, so the panel's **Add** button disappears at eight types. Shelf and panel write the same `FoliagePaintSettings` — click either.

### Foliage type settings

A type's geometry is described by five numbers in the panel's **Properties** section. The first two multiply, and together they are what decides whether the ground shows through:

| Setting | Range | What it does |
|---|---|---|
| **Density** | `1`–`128` | Scatter **clumps** per square metre (default `48`). This is the grid, not the blade count. |
| **Blades / Clump** | `1`–`16` | Blades grown from each scatter point (default `5`). One blade per grid cell always leaves visible ground between cells — grass reads as grass when it comes in tufts, and a tuft is much cheaper than a finer grid because the whole clump shares one density lookup. |
| **Height Range** | `0.01`–`2` | Min/max blade height, in metres. |
| **Width Range** | `0.002`–`0.5` | Min/max blade width, in metres. This is the blade's *actual* width — it is not scaled by the height. |
| **Wind Strength** | `0`–`2` | Amplitude of the vertex-shader wind. |

Blades per square metre is `Density × Blades / Clump` — `240` at the defaults, which a chunk painted wall to wall carries at full density. A chunk is still capped, at **2,000,000 blades** (~490/m² over a 64 m chunk); past that the scatter thins uniformly, so density degrades rather than frame rate. The cap is a backstop against a pathological config, not a limit normal use should meet.

The painted weight sets coverage **in proportion**, with no floor under it: half the paint scatters half the blades, all the way down to bare ground. That is what keeps a patch inside the brush — the brush's own falloff leaves a weight gradient at the rim, which becomes a density gradient rather than a hard circle somewhere outside the gizmo. Coverage tops out at weight `0.25`, well before `1.0`, because a moving stroke never drives a texel near `1.0`.

The paint mask itself is sized in **world units** — 4 texels per metre, capped at 256² per chunk — and sampled bilinearly. A fixed 64² mask gave a 64 m chunk one texel per metre, and since the smallest brush is 0.01 of a chunk side (0.64 m on a 64 m chunk), it painted a single texel that then rendered as a hard 1 m square of grass around a 1.3 m gizmo. Chunks in scenes saved before this keep the mask they were saved with; only new chunks get the finer one.

> A `TerrainFoliageConfig` component (splatmap-weighted auto-scatter of arbitrary meshes) still exists as a registered type, but no system currently consumes it — hand-painted density is the supported foliage path.

## Components & scene format

Terrain is serialized into the RON scene like any other entity (see [Scenes & Hierarchy](/docs/r1-alpha7/editor/scenes)). The meaningful, `Reflect`-serialized fields:

```ron
// Root terrain entity
TerrainData(
    chunks_x: 1,
    chunks_z: 1,
    chunk_size: 64.0,        // metres per chunk side
    chunk_resolution: 129,   // vertices per side
    max_height: 40.0,
    min_height: -10.0,
),

// One per chunk child (base_heights normalized 0..1, row-major)
TerrainChunkData(
    chunk_x: 0,
    chunk_z: 0,
    base_heights: [ /* chunk_resolution² floats */ ],
),

// On the root terrain entity: the paint layer stack
Painter(
    layers: [ /* up to 8 PaintLayer: name, material_path, mask, height_offset, … */ ],
    active_layer: Some(0),
),
```

The composed `heights` buffer, the per-layer overlay meshes, and the trimesh collider are all runtime-only — they're rebuilt on load, so they aren't written to the scene.
