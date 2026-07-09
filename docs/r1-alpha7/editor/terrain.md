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

A fresh terrain is a **single 64 m × 64 m tile** at `chunk_resolution = 129` (129 × 129 vertices), with a checkerboard placeholder material and a flat surface sitting on the editor grid plane. The default height range is `-10 m` to `40 m`.

> Chunks each get a **trimesh collider** (`renzora_physics`'s `CollisionShapeData::mesh()`), rebuilt from the chunk mesh whenever you sculpt — so the avian physics collider always matches the visible surface. It is a triangle mesh, *not* a heightfield collider.

## The Terrain Tools panel

The **Terrain Tools** panel (panel id `terrain_tools`, category "Terrain" in the dock's panel picker) has a full-width **Enable Terrain Mode** toggle at the top, then two tabs: **Sculpt** and **Paint**.

The panel and the viewport toolbar drive the same state, from either direction:

- Toggling **Enable Terrain Mode** on selects the first terrain and arms the current tab's tool; toggling it off returns to **Select**.
- Clicking the panel's **Sculpt**/**Paint** tab switches the active tool with it.
- Activating a terrain tool from the viewport toolbar reveals the panel body and switches its tab to match.

Three toolbar buttons appear in the viewport toolbar whenever a terrain exists in the scene (section "Terrain"):

| Button | Tool | Inspector tab |
|--------|------|---------------|
| **Sculpt Terrain** | `TerrainSculpt` | Sculpt |
| **Paint Terrain Layers** | `TerrainPaint` | Paint |
| **Paint Foliage** | `FoliagePaint` | — (Foliage panel) |

Clicking a button selects the first terrain, switches the tab, and activates the tool. Click the active button again to return to the **Select** tool.

## Sculpting

On the **Sculpt** tab, pick a brush from the 16-tool grid, then paint in the viewport. The brush position is found by **mesh raycast** against the actual sculpted surface, so the gizmo hugs the terrain. Hold the left mouse button and drag to sculpt continuously.

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

> A **Stamp** brush exists in `TerrainBrushType` (stamping a grayscale image or a procedural preset — Dome, Cone, Bell, Mesa, Ridge, Crater, Noise — with Add / Subtract / Replace / Max / Min blending), but it is not currently surfaced in the native tool grid.

### Tool Settings

- **Strength** (`0.01`–`1.0`) — always shown.
- **Flatten** brush adds a **Mode** combo (Both / Raise / Lower) and a **Target Height** drag (`0`–`1`).
- **Noise** brush adds **Mode**, **Scale**, **Octaves**, **Lacunarity**, **Persistence**, **Seed**, and (in Warped mode) **Warp** strength.
- **Terrace** brush adds **Steps** and **Sharpness**.

### Brush Settings

- **Size** — brush radius in **world metres** (`1`–`200`). The scroll wheel resizes it (×1.1 / ×0.9) while hovering the viewport.
- **Falloff** (`0`–`1`) — how far the soft edge reaches in from the rim.
- **Shape** — **Circle**, **Square**, or **Diamond**.
- **Falloff Type** — **Smooth** (cosine), **Linear**, **Spherical**, **Tip**, or **Flat**.

The gizmo draws an outer ring plus an inner falloff ring (and a vertex-density grid preview for the Stamp brush).

### Undo / redo

Sculpt and paint strokes are snapshotted on mouse-down and recorded onto the editor's central **Scene undo stack** on mouse-up, so they appear in the History panel alongside every other scene edit. **Ctrl+Z** undoes, **Ctrl+Y** (or **Ctrl+Shift+Z**) redoes.

## Heightmap import / export

The Sculpt tab's **Heightmap Import** section has **Import Heightmap…** and **Export Heightmap…** buttons.

- **Import** accepts **8- or 16-bit grayscale PNG** (8-bit RGB/RGBA use the red channel) or **RAW16** (`.r16` / `.raw`, 16-bit unsigned, row-major). The image is bilinearly resampled across every chunk and written into `base_heights`.
- **Export** writes a single **16-bit grayscale PNG** of the composed heightmap across all chunks.

## Painting layers

The **Paint** tab paints coverage masks into the terrain's **`Painter`** component — a stack of paint layers on the terrain root entity. Pick a paint mode from the 4-tool grid:

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

> A `TerrainFoliageConfig` component (splatmap-weighted auto-scatter of arbitrary meshes) still exists as a registered type, but no system currently consumes it — hand-painted density is the supported foliage path.

## Components & scene format

Terrain is serialized into the RON scene like any other entity (see [Scenes & Hierarchy](/docs/r1-alpha5/editor/scenes)). The meaningful, `Reflect`-serialized fields:

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
