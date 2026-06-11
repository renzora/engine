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

Selecting a terrain and activating a terrain tool opens the **Terrain Tools** panel (panel id `terrain_tools`). It has a single full-width **Enable Terrain Mode** toggle at the top, then two tabs: **Sculpt** and **Paint**.

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

Sculpt and paint strokes are snapshotted on mouse-down and pushed to a dedicated terrain undo stack on mouse-up. **Ctrl+Z** undoes, **Ctrl+Y** (or **Ctrl+Shift+Z**) redoes — while any terrain tool is active.

## Heightmap import / export

The Sculpt tab's **Heightmap Import** section has **Import Heightmap…** and **Export Heightmap…** buttons.

- **Import** accepts **8- or 16-bit grayscale PNG** (8-bit RGB/RGBA use the red channel) or **RAW16** (`.r16` / `.raw`, 16-bit unsigned, row-major). The image is bilinearly resampled across every chunk and written into `base_heights`.
- **Export** writes a single **16-bit grayscale PNG** of the composed heightmap across all chunks.

## Painting layers

The **Paint** tab paints per-texel material weights into a splatmap. Pick a paint mode from the 4-tool grid:

| Tool | Effect |
|------|--------|
| **Paint** | Stamp the active layer's coverage (idempotent — overlapping strokes don't amplify) |
| **Erase** | Remove the active layer's coverage |
| **Smooth** | Blur the active layer's mask against itself |
| **Fill** | Set coverage to full under the brush |

### Layers

A paintable surface holds **up to 8 material layers** (`MAX_LAYERS = 8`); **layer 0 is the terrain base**. New surfaces start with four defaults — **Grass**, **Dirt**, **Water**, **Rock**.

Painting is **non-destructive**: each layer keeps its own coverage **mask**, and the GPU splatmap is composed **top-down** — each upper layer claims `min(mask, remaining coverage)` of a texel and layer 0 absorbs whatever is left. Disabling or deleting a layer just hides its mask; the authored data survives.

In the **Layers** section: click a row to select the active layer, use **Add Layer** (hidden once 8 layers exist), and drop a **`.material`** asset onto the active layer's drop zone to drive its appearance (albedo / normal / ARM texture paths are extracted from the material graph). The ✕ clears the assignment. Each `MaterialLayer` also carries a `carve_depth` — a normalized height delta applied where its mask is full, composed into the chunk heights so carving a path doesn't fight the base sculpt.

### Paint Brush Settings

- **Size** (`0.01`–`0.5`) — brush radius in **UV fraction** of a chunk (the scroll wheel resizes within that range).
- **Strength** (`0.01`–`1.0`), **Falloff** (`0`–`1`), and **Shape** (Circle / Square / Diamond).

> The splatmap resolution defaults to **256 × 256** per chunk and is independent of the heightmap resolution.

## Foliage

Foliage is the separate **Paint Foliage** tool (`renzora_foliage_editor`, panel id `foliage_painting`) — not part of the Terrain Tools panel, which only links to it. Foliage scatters instanced meshes onto the surface, weighted by a paint layer.

Configure scattering with a `TerrainFoliageConfig` component:

| Field | Meaning |
|-------|---------|
| `layer_index` | Which paint layer drives placement |
| `density` | Instances per square unit |
| `min_weight` | Minimum splatmap weight to place at all |
| `mesh_path` | Foliage mesh asset path |
| `material_path` | Foliage material asset path |
| `height_range` / `width_range` | Random min/max blade height and width |
| `random_rotation` | Random Y-axis rotation |
| `align_to_normal` | Orient instances to the surface normal |
| `enabled` | Turn the config on/off |

Instances are auto-scattered from the chunk's splatmap weights and respawned when the splatmap or config changes.

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

// Optional, per chunk: the paint splatmap
PaintableSurfaceData(
    layers: [ /* up to 8 MaterialLayer */ ],
    splatmap_resolution: 256,
),
```

The composed `heights` buffer, the derived `splatmap_weights`, and the trimesh collider are all runtime-only — they're rebuilt on load, so they aren't written to the scene.
