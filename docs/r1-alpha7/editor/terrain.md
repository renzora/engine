# Terrain

Generate, sculpt and paint heightmap terrain in the editor with live gizmos, then save it straight into your RON scene.

## How it works

Terrain is two crates working together:

- **`renzora_terrain`** — the runtime. `TerrainPlugin` registers the data types, builds chunk meshes, composes heightmaps, uploads splatmaps, and scatters foliage. It self-registers with `renzora::add!(TerrainPlugin)`, so terrain renders in **both the editor and your shipped game**.
- **`renzora_terrain_editor`** — the editor-only tools (`TerrainEditorPlugin`, `Editor` scope): the brush gizmo, the [Generate](#generating-a-landscape) region gizmo, sculpt/paint systems, the **Terrain Tools** panel, undo/redo, and heightmap import/export. Foliage painting is a separate editor crate, `renzora_foliage_editor`.

A terrain is a **parent entity** (`TerrainData`) with one **chunk child** (`TerrainChunkData`) per tile. Each chunk stores a square grid of heights normalized to `[0, 1]`; the chunk's `TerrainData` maps that range onto world `min_height..max_height`. Sculpting writes the chunk's `base_heights`; a composition pass adds any per-layer carve deltas to produce the final `heights` the mesh and collider read.

> There is **no terrain scripting API**. Older docs showed `terrain_get_height(x, z)` / `terrain_set_height(x, z, h)` and globals like `position_x` — none of these exist in Lua or Rhai. Terrain is authored in the editor and serialized into the scene; runtime height queries are done with a standard mesh raycast.

## Creating terrain

Add a terrain from the editor's **Add** menu (it's registered as the `terrain` entity preset, icon "mountains", category "general"), or click any terrain toolbar button while no terrain exists. You can also [turn a plane you have already placed into one](#making-a-terrain-out-of-a-plane).

A fresh terrain is a **single 64 m × 64 m tile** at `chunk_resolution = 129` (129 × 129 vertices), with a checkerboard placeholder material and a flat surface sitting just above the editor grid plane. The default height range is `-10 m` to `40 m`.

The parent entity spawns at `y = 0.05` rather than `y = 0`. A new terrain's flat surface would otherwise be exactly coplanar with the grid, and the grid — which draws in the transparent pass without writing depth — breaks up into shimmering patches against a surface at its own depth. The 5 cm lift puts the terrain strictly in front, so it occludes the grid cleanly. It's an ordinary Transform, saved with the scene: move the terrain to `y = 0` yourself if you want it exactly on the plane.

### Making a terrain out of a plane

You do not have to decide up front. Select **any flat mesh** — a Plane primitive, a subdivided grid, an imported ground plane — and a **Make Terrain** button (icon: shovel) appears in the terrain row of the viewport's top strip. Click it and that entity becomes a terrain in place, then drops straight into the Sculpt brush so you can start shaping it.

It stays the *same entity*: same name, same place in the hierarchy, same scripts and components, same spot in the scene. What changes is its geometry — the single flat mesh is replaced by the chunk grid the brushes work on.

- **"Flat" is about shape, not origin.** A mesh qualifies when its bounds are thin in Y next to their X/Z footprint (within 2%), so it doesn't matter whether it was spawned from the Plane shape or imported. A cube, a wall or a hillside won't offer the button.
- **The terrain starts flat.** The heightmap is a fresh one at the plane's level — any millimetre-scale relief already in the mesh is not sampled into it. Convert first, then shape with the brushes or [Generate](#generating-a-landscape).
- **The grid matches the ground the plane covered.** The chunk size is taken from the plane's *shorter* side (aiming for the usual 64 m, but never larger than the plane), and the longer side is tiled with however many of those chunks it takes — so a 100 × 20 plane becomes a 5 × 1 terrain of 20 m chunks, not a 100 × 50 one. Resolution follows the chunk size, aiming for roughly half-metre vertex spacing, so a small plane doesn't get centimetre triangles. A plane scaled out to kilometres is capped at 64 chunks, with the chunk size growing to cover it.
- **Scale and rotation are baked in.** The terrain keeps the plane's position, but its own transform comes back to identity rotation and scale 1 (including any inherited from a parent), with the size folded into the chunk size instead. The brushes map a cursor hit into heightmap coordinates through the terrain root's *position* alone, so a rotated or scaled terrain root would land every stroke somewhere other than under the pointer.
- **Height range and surface.** The new terrain gets the standard `-10 m` to `40 m` range, whose flat level sits at local `y = 0` — exactly where the plane's surface was. Unlike a terrain added from the **Add** menu it is *not* nudged up by 5 cm to clear the editor grid: it is replacing a mesh you already placed, so it stays where you put it.
- **Materials.** A `.material` assigned to the plane carries over to every chunk. A plane wearing only a plain colour gets the terrain checkerboard, the same as a fresh terrain.
- **Undoable.** Ctrl+Z puts the plane's mesh and transform back and clears the chunks away. (Sculpting done after the conversion lives on those chunks, so undoing the conversion itself discards it.)

Flat meshes that are *part of an imported model* don't offer the button — the model respawns its own children from the source file, so the plane would come straight back next to the new terrain.

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

The **Resize Terrain** shelf button turns on the Region tool, which grows and shrinks the terrain by clicking in the scene rather than by typing numbers.

- Ghost tiles ring the terrain, each marked with a `+`. **Click one** to grow the terrain to meet it.
- **Ctrl+click** an edge tile to remove that whole row or column; the row that would go is highlighted in red first.
- Only edge-adjacent ghosts are offered — there are no diagonal ghosts, since a rectangle can't grow "diagonally" without adding two rows at once.

The chunk grid is centred on its parent, so changing the count re-centres every chunk. The tool compensates for that: growing on the −X or −Z side re-indexes the surviving chunks and shifts the terrain's transform by half a chunk the other way, so **terrain you have already sculpted does not move**. That arithmetic lives in `renzora_terrain::grid` and is unit-tested on all four edges in both directions.

> The grid stays a dense rectangle. Sparse, non-contiguous regions would be a rewrite of `TerrainData`, the chunk addressing and the scene format.

## Generating a landscape

The **Generate Terrain** button at the top of the [tool shelf](#the-tool-shelf-and-the-brush-bar) (icon: magic wand) turns on the Generate tool — a procedural mountain generator you aim with a gizmo instead of a brush. It sits with **Resize Terrain** and **Terrain Size & Resolution**: the three operations that act on the terrain as a whole rather than on the patch under a brush. Like the rest of the shelf they appear once any terrain mode is in hand, so it's one hop — click Sculpt on the toolbar and the column comes up with Generate at the top of it.

Everything it does is decided by one parameter set evaluated over one rectangle, so the result is a function of the settings and nothing else. Change a number and press Generate again and you get the same landscape with that one thing different — which is the whole reason it exists alongside the [Noise brush](#sculpting), whose result depends on where your strokes happened to overlap.

### The region gizmo

A blue rectangle sits on the terrain's ground plane, with a handle at each corner and each edge midpoint, and vertical posts running up to the surface above it.

- **Drag a corner** to resize from that corner — the opposite one stays put.
- **Drag an edge midpoint** to move just that edge.
- **Drag inside the rectangle** to move the whole region without resizing it.

The region starts as the whole terrain and follows it as you resize the terrain with the [Region tool](#resizing-with-the-region-tool). The first handle drag pins it to an explicit rectangle, after which it stays where you put it.

Handles are picked against the terrain's flat ground plane rather than against the sculpted surface — a grab point that slid down a hillside as you dragged would make the rectangle impossible to place. A drag that runs off the terrain is clamped to it, and a drag that would collapse the rectangle to a line stops at a four-vertex minimum so the handles stay grabbable.

### The preview

Above the rectangle, a wireframe shows the surface the current settings would produce, blended against the heights that are already there. **Nothing is written until you press Generate** — every slider on the bar is preview-only until then.

The preview and the apply pass call the same `renzora_terrain::generate::blended_height`, so it is the result rather than an approximation of it. Its brightness follows the region weight, which makes the **Feather** band visible as the preview fading into the ground instead of a number you have to imagine.

Turn the wireframe off with the bar's **Preview** switch if it's in the way; the rectangle and handles stay.

### Settings

They sit in their own bar across the top of the scene, the same surface the brush settings use, and they appear only while the Generate tool is on.

| Setting | What it does |
|---|---|
| **Noise** | FBM / Ridge / Billow / Warped / Hybrid. **Hybrid** is the default — ridged noise with FBM mixed back in, which gives sharp crest lines with flanks that don't look machined. |
| **Scale** | Metres per unit of noise: the size of the largest features. The dial that decides "alpine range" from "gravel". Set it wider than the terrain to get one mountain rather than a range. |
| **Oct** | Octaves, 1–8. More detail per doubling of frequency. |
| **Rough** | Persistence — how much each octave contributes relative to the one before. |
| **Peaks** | Raises the noise to this power before it becomes a height. Above 1 it pushes the midrange down, turning rolling lumps into peaks separated by valley floors; below 1 it flattens the tops into plateaus. |
| **Height** | Peak-to-floor amplitude in world metres. Capped by the terrain's own `max_height - min_height` — asking for 500 m on a 50 m terrain gives you 50, not a plateau of clipped values. |
| **Base** | Elevation the noise floor sits at, in world metres. |
| **Blend** | How the result meets what is already there: Replace / Add / Subtract / Max / Min. **Replace** is the default, for the usual case of a flat terrain you just spawned. |
| **Feather** | Width of the edge blend, as a fraction of the region's half-extent. 0 is a hard edge. It blends toward whatever is already there, not toward a fixed level, so a region generated over an existing hillside still lands on that hillside. |
| **Seed** | The landscape. |

Three buttons finish the bar:

- **Re-roll** advances the seed by one. Nothing is written — the preview just becomes a different landscape, so you can walk through them until one looks right. Stepping rather than randomising means you can walk back to the one you passed.
- **Generate** commits it, as **one** undo step labelled "Generate Terrain".
- **Flatten** levels the terrain's base layer to the **Base** height, as its own undo step. It's the forwards way out of a generate you don't like.

### What it writes

Generate writes each chunk's `base_heights` — the same layer the sculpt brushes write — so a generated landscape is something the brushes then carve into, and the height-layer stack composes on top of it exactly as it does for hand-sculpted ground. Chunks the region never touches are skipped whole and never flagged for a mesh rebuild.

The generator's maths lives in `renzora_terrain::generate` and takes no `World`, so it is unit-tested directly; the tool in `renzora_terrain_editor::generate_tool` is only the cursor-to-rectangle part.

## The tool shelf and the brush bar

With a terrain tool active, the brushes live on the **tool shelf** — a two-column palette floating down the viewport's left edge, in the shape image editors use. At the top of it sits the whole-terrain group — **Generate Terrain**, **Resize Terrain**, **Terrain Size & Resolution** — and below that the palette for the current mode. The sculpt palette shows all 17 sculpt brushes; switching to the paint tool swaps it for the 4 paint brushes, and switching to Paint Foliage swaps it for the [foliage palette](#the-foliage-shelf). The shelf collapses entirely when no shelf tool applies.

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

Pick a brush from the shelf (or the panel's grid), then paint in the viewport. The brush position is found by **mesh raycast** against the actual sculpted surface, so the [cursor hugs the terrain](#the-brush-cursor). Hold the left mouse button and drag to sculpt continuously.

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

### The brush cursor

Both terrain brush tools draw the same cursor, and both find it the same way: a **mesh raycast** against the chunk meshes, filtered to chunks only so paint-layer overlays and grass don't swallow the ray and leave the brush dead over ground you've already worked on.

The cursor is two rings, and both ride the surface — every point around them samples the heightmap, so the cursor lies on a hillside instead of hovering flat above it:

- the **outer ring** at the brush radius, drawn in the shape you picked (circle, square or diamond);
- the **inner ring** at the edge of the full-strength core — the gap between the two is the falloff band, so you can see how soft the brush is rather than reading it off a slider.

The colour says which brush is in hand.

> Paint used to draw a flat circle here that ignored both the shape and the falloff it lets you set, which meant discovering the brush by painting and undoing. It draws the shared cursor now. The code is `renzora_terrain_editor::brush_gizmo`; the **Stamp** brush adds its wireframe grid preview on top of the same outer ring.

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

- **Size** (`0.01`–`0.5`) — brush radius as a **fraction of a chunk side** (the scroll wheel resizes within that range), so the brush scales with the terrain rather than with a metre count.
- **Strength** (`0.01`–`1.0`), **Falloff** (`0`–`1`), and **Shape** (Circle / Square / Diamond).

All three show in the [brush cursor](#the-brush-cursor), which is the same surface-following one the sculpt brushes use.

## Foliage

Foliage is the separate **Paint Foliage** tool (`renzora_foliage_editor`, panel id `foliage_painting`) — not part of the Terrain Tools panel, which only links to it. You paint a per-chunk **density map** (`FoliageDensityMap`), and the runtime bakes animated grass blades into the painted areas, re-baking as you sculpt underneath. The density map serializes with the scene.

Grass appears **as you drag**. A rebuild rescatters the chunk's entire blade set, so the preview runs on a duty cycle paced by what that actually costs (`FoliageRebuildCost`, measured live): a bare chunk previews every frame, an expensive one backs off to as slow as 2 Hz, and every chunk the stroke touched is rebuilt once more on release so the result never ends on a stale preview. Since blades became GPU instances the scatter no longer builds geometry, so most strokes now sit at the fastest interval.

The scatter runs **in parallel across grid rows** (`ComputeTaskPool`, banded so the pool isn't handed ~450 microtasks). A 64 m chunk painted wall to wall asks for roughly a million blades, and regenerating those on the main thread is what held the preview to a visible stutter — the pacing keys off measured cost, so a cheaper rebuild speeds the preview up on its own. Bands are concatenated in spawn order, which reproduces the serial scatter exactly: the blade order is the order the instance buffer uploads in, and a result that depended on thread timing would reshuffle the field on every rebuild. Below `PARALLEL_SCATTER_THRESHOLD` (20 K expected blades) it stays on the calling thread, since scheduling a small stroke costs more than scattering it.

### How grass renders

Grass draws through its **own instanced render pipeline** (`renzora_terrain::foliage::render`), not through a Bevy `Material`. One draw call per chunk per foliage type, `24` vertices by however many blades that chunk scattered.

The blade's *shape* is rebuilt in the vertex shader from `@builtin(vertex_index)`, so it costs no memory at all; the only per-blade data that reaches the GPU is a 48-byte instance record — position, height, width, wind phase, bend, lean, and the sine/cosine of its rotation. The previous design baked every blade into one giant mesh per chunk at roughly **560 bytes a blade**, which is what forced a low blade budget, made repainting expensive, and put a ceiling on density. Instancing is a **~12×** reduction, and it is why the budget could go from 250 K blades to 2 M.

A rebuild **updates the chunk's batch entity in place** rather than despawning and respawning it. That is not just an optimisation: `queue_grass` only enqueues chunks that already carry a `GrassInstanceBuffer`, and that buffer is created in the render schedule's Prepare — which runs *after* Queue. A freshly spawned batch entity is therefore invisible for the whole first frame of its life, so respawning one on every preview tick made the entire painted area strobe while you dragged. Reuse also restores the intended upload path: the render world keeps one GPU buffer per chunk and refills it only when `BladeSetId` changes, instead of reallocating on every stroke.

Blades are drawn **double-sided**. A back-face-culled blade simply vanishes when you walk behind it — about half of them at any moment — and the fragment shader flips the normal so the back face is still lit rather than reading as a black cut-out.

> **Grass does not cast shadows and does not write into the depth prepass.** The old baked mesh was an ordinary `Material`, so it landed in the shadow and prepass pipelines for free; a hand-written pipeline does not. Depth-driven effects (SSAO, contact shadows) currently see through grass. Restoring it means a second pipeline running the same vertex expansion against the `Shadow` and prepass phases. Grass is also absent from the viewport's debug visualization modes for the same reason — there is no material asset to swap.

### The foliage shelf

With **Paint Foliage** active, the same left-edge [tool shelf](#the-tool-shelf-and-the-brush-bar) the terrain brushes use swaps to the foliage palette, in two groups:

- **Paint / Erase / Grow / Trim** — the brush mode. Paint and Erase work the active type's density; [Grow and Trim](#painting-grass-height) work the chunk's blade height.
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

### Painting grass height

`Height Range` above sets how tall a type's blades are *everywhere*. **Grow** and **Trim** vary that across the ground: they paint a per-texel **height multiplier** into the chunk, and every blade scattered there is scaled by it. Unpainted ground is `1.0`.

Both aim at the Brush section's **Height** slider (`0.25`–`3.0`, default `2.0`), approaching it at the brush's Strength and through its falloff — exactly the way Paint and Erase approach `1.0` and `0.0`. The slider is what makes them precise rather than merely directional: set it to `1.0` and Trim returns grown ground to *exactly* neutral. Direction still decides which brush can act — Grow only raises, Trim only lowers — so where two strokes overlap, neither undoes the other.

Use it for a meadow that thickens into long grass in the hollows, a mown lawn with rough at the edges, or a worn path that stays cropped without going bald.

Three things to know:

- **The multiplier is per chunk, not per foliage type.** Height is a property of the ground — a sheltered hollow grows everything in it taller — so growing an area grows every type painted there. A per-type copy would carry 8 floats per texel and double a map that already reaches 2 MB a chunk in the scene file.
- **It changes height, not coverage.** Trimming shortens blades without removing any; the density mask is still the tool for cutting a gap. `0.25` is a mown lawn, not bare earth, which is why Trim's floor isn't zero.
- **Blade width follows at the square root.** A blade three times taller and exactly as wide reads as a wire, three times wider reads as a leaf; the half-power keeps grown grass looking like the same plant, only bigger.

The channel is **allocated lazily** — a chunk nobody has run a height brush over stores nothing and serializes nothing, and a scene saved before the feature existed loads as neutral everywhere.

> **Related:** blade wind deflection now scales with blade length, normalised so the default `0.1`–`0.4` height range keeps the motion it already had. It didn't before — the displacement was in absolute metres, so a taller blade travelled the same distance and read as stiffer. That was invisible while every blade of a type sat inside one narrow authored range, and obvious the moment Grow could put a 3× blade next to a neutral one. See [Wind](../rendering/wind.md).

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
