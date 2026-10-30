# Terrain

Generate, sculpt and paint heightmap terrain in the editor, then save it into your scene. Terrain renders in the editor and in your shipped game alike.

## Creating terrain

Add one from **+ Add Entity**.

A fresh terrain is a single 64 metre tile with a flat surface just above the editor grid, wearing the same blockout grid as a fresh shape, one tile to the metre. Its default height range is -10 to +40 metres. Give it a [landscape material](#the-landscape-material) to texture it: **Use Alpine Landscape** in the Terrain component's material slot gives grass, with gravel, rock and snow appearing as you shape it.

### Turning a plane into terrain

You do not have to decide up front. Select any flat mesh, a plane primitive or an imported ground plane, and a **Make Terrain** button appears in the terrain row of the viewport toolbar. Click it and that entity becomes a terrain in place, then drops straight into the Sculpt brush.

It stays the same entity: same name, same place in the hierarchy, same scripts and components. Only its geometry changes.

A few things follow:

- **Flat means shape, not origin.** A mesh qualifies when its bounds are thin in Y next to their footprint. A cube or a wall will not offer the button.
- **The terrain starts flat.** Any millimetre-scale relief already in the mesh is not sampled. Convert first, then shape.
- **The grid matches the ground the plane covered.** Chunk size comes from the plane's shorter side, and the longer side is tiled, so a 100 by 20 plane becomes a 5 by 1 terrain of 20 metre chunks.
- **Scale and rotation are baked in.** The terrain keeps the plane's position but comes back to identity rotation and scale 1, with the size folded into the chunk size instead.
- **`Ctrl+Z` puts the plane back.**

Flat meshes that are part of an imported model do not offer the button, because the model respawns its own children from the source file.

## The Terrain component

Select the terrain root and the Inspector shows one **Terrain** section. From the top:

1. **The material slot**: the terrain's [landscape material](#the-landscape-material), with its thumbnail, path and a summary ("4 layers, height blend"). **Edit** opens it in the Material Editor, **Choose...** picks a `.material` from the project, and you can drop a `.material` from the Assets panel onto the slot. A terrain without one wears the blockout grid, and the slot offers **Use Alpine Landscape** in one click.
2. **The mode tabs**: **Manage**, **Sculpt**, **Paint** and **Foliage**. Each shows only what that mode needs. Sculpt, Paint and Foliage arm their brush in the viewport; Manage goes back to Select. Picking the move gizmo disarms a brush but leaves the tab where it was, so you do not lose your place.

<!-- screenshot: terrain_inspector.png - the Terrain component with its material slot and mode tabs -->

Every value is in real units: sizes and heights in metres, angles in degrees, strengths and weights in percent.

### Manage

The whole terrain in one place. It replaces the old **Generate**, **Resize** and **Edit Terrain...** buttons, which each opened a different kind of UI.

**The chunk map** is the terrain seen from above, one cell per chunk: bright for chunks at full detail, dim for chunks [streamed out](#streaming-and-detail) to a stand-in, with the camera's cell outlined. Empty cells ring the grid.

- **Click an empty cell beside an edge** to add that row or column. The terrain you have already sculpted stays exactly where it is.
- **Ctrl+click an edge chunk** to remove its row or column. Removal deletes the sculpting and paint on it and is not an undo step, which is why it needs Ctrl. Hovering shows the whole row a click would add (accent) or remove (red).

Below the map: the terrain's **Size**, how many chunks are **Resident** at full detail, and an estimate of the **Memory** they use. The grid runs up to 32 chunks a side.

**The settings card**:

| Setting | Range | Applies |
|---|---|---|
| Chunk size | 8 to 512 m | On **Apply** |
| Resolution | 33, 65, 129 or 257 vertices per side | On **Apply** |
| Height range | Min and max, in metres | Live |
| Streaming | On or off, and the radius in metres | Live |
| Distant detail | Levels (0 to 6) and reach (percent of the default distances) | Live |

Chunk size and resolution rebuild every chunk with a fresh collider, so they wait: **Apply** and **Revert** appear once one of them differs from the terrain, with the vertex count and memory the new values would cost, and a warning past two million vertices. Resolution has fixed steps so neighbouring chunk edges share vertices exactly.

**Min and Max Height are an envelope, not a height.** They define the band sculpting can reach, and heights are stored normalized inside it. Changing either **rescales the ground you already have**, because a vertex stored at 0.2 sits at a different world height once the envelope moves. Set the envelope before you sculpt, rather than reaching for it later to make mountains taller. To see how much of the band you are using, read the **Height** and **Range** rows of the [statistics overlay](/docs/r1-alpha8/editor/viewport#statistics).

**The Generator card** is the terrain's starting shape: a `.tgraph` [terrain graph](/docs/r1-alpha8/editor/terrain-graph) of primitives, erosion and masks. Drop a `.tgraph` on its slot or **Choose...** one; the slot shows the graph's name, path, node count and when it was last built, and **Edit** opens it in the Terrain workspace. With a graph set:

- **Seed**, with **Re-roll** for a new one. One graph with two seeds is two different landscapes of the same kind.
- The graph's **exposed parameters**, such as Highlands' peak height, each overriding the file's value for this terrain only.
- **Build** regenerates the ground in the background, with a progress bar and **Cancel**. Your sculpting lives on its own layer above the generator, so a rebuild keeps it.

The engine ships Highlands, Rolling Hills and Desert graphs on the Assets panel's create menu.

**Whole terrain**: **Erode all** weathers the whole terrain (rain carves gullies and fills valleys with sediment, then slopes settle to their angle of repose), and **Flatten all** levels the sculpt layer to 0 m. Each is one undo step. Erosion works the same at any grid spacing: on a fine terrain the raindrops run on a grid of about 3 m and the change is carried back up to full resolution, so gullies come out a few metres across instead of a comb of one-vertex grooves.

**Heightmap import and export** is folded away at the bottom; see [Heightmaps](#heightmaps).

### Streaming and detail

| Setting | What it does |
|---|---|
| Streaming | Keep full detail only within the radius of the camera, the viewport's while editing; farther chunks are drawn by a coarse stand-in. On, at 512 m, for a new terrain (see [Streaming](../engine-core/streaming.md#terrain-chunk-streaming)) |
| Distant detail: levels | How many coarser versions of each chunk are drawn at a distance, 0 to 6 (see [Level of detail](#level-of-detail)) |
| Distant detail: reach | Scales the distances at which they take over: above 100% keeps detail farther out |

### Virtual geometry

With virtual geometry on (**Project Settings > Rendering**, on by default) and a [landscape material](#the-landscape-material) on the terrain, each chunk is drawn as [virtual geometry](../rendering/virtual-geometry.md): cut into small clusters, each drawn at the detail its distance needs, so the ground keeps its full detail up close and thins out smoothly with distance, with no levels switching and no seams.

The editor builds each chunk's clusters in the background once its ground has stopped changing for about three quarters of a second, and saves them as `terrain_clusters/<id>/chunk_<x>_<z>.rvg` in the project (the asset browser hides that folder). While a brush is changing a chunk, and until its new clusters are ready, the chunk is drawn the ordinary way, with the levels of detail below. An exported game ships the files and loads them; it never builds clusters itself, and a chunk whose file is missing or out of date is drawn the ordinary way.

Physics, shadow maps and Solari read each chunk's ordinary full-detail mesh whether or not it is drawn as virtual geometry. A terrain with the default blockout grid (no material) is always drawn the ordinary way.

### Level of detail

Where a chunk is not drawn as virtual geometry (virtual geometry off, on the web, no material, or its clusters not built yet), it is drawn at full detail up close and with fewer triangles as it gets farther away, so a large terrain costs little more to draw than a small one. Every level has half the vertices per side of the one before (a quarter of the triangles). With the default four levels and 64 metre chunks:

| Distance to the chunk's centre | Drawn with |
|---|---|
| Under 192 m | Full detail |
| 192 to 384 m | 1/4 of the triangles |
| 384 to 768 m | 1/16 |
| 768 to 1536 m | 1/64 |
| Beyond 1536 m | 1/256 |

The distances scale with the chunk size and with **Distant detail: reach**. Coarser levels keep the full-detail lighting, so switching changes the outline of distant hills rather than their shading. A coarse chunk keeps its edges at full detail, and its edge normals are taken across the seam from the neighbouring chunk, so it meets a neighbour at any level exactly, with no gap and no line of different shading between them.

A sun still on the default shadow cascades (150 m) has them stretched to cover the terrain, one and a half times its diagonal up to 3 km, so the whole terrain keeps its shadows when you pull back to see it. Once you set a sun's cascades yourself, the terrain leaves them alone.

Physics, the brushes and the material's slope and height rules always use the full-detail ground: the levels only change what is drawn.

The grid size is never a live field: each write of it respawns every chunk, so dragging the grid from 1 to 8 would build every size in between. That is why the chunk map adds or removes one row at a time and chunk size and resolution wait for Apply.

## Sculpting

Open the **Sculpt** tab and pick a brush: that also arms sculpting in the viewport. Then drag in the viewport.

The brushes come in two labelled groups: **Shape** (Sculpt, Raise, Lower, Flatten, Set Height, Ramp, Smooth, Relax, Pinch, Stamp, Erase, Retop) and **Weather and detail** (Noise, Erode, Rain, Terrace, Cliff). Below them, one card shows the chosen brush with a line on what it does and only the settings that brush reads; below that, one **Brush** card that every sculpt brush shares.

| Brush | What it does | With `Shift` |
|---|---|---|
| Sculpt | Raise the terrain under the brush | Lower |
| Raise | Push up | |
| Lower | Push down | |
| Smooth | Blur the ground at a scale set by the brush: a tenth of the radius, so a large brush rounds off large features, not just single vertices | |
| Flatten | Level toward the height where the stroke began, with a shoulder no steeper than 40 degrees back to the ground around it | |
| Set Height | Ease toward a target height, with the same 40 degree shoulder, so a dug lake gets banks, not quarry walls | |
| Erase | Reset toward the flat baseline | |
| Noise | Add fractal noise | Smooth |
| Terrace | Cut level treads with sloped risers between them, each step the set height in metres, with wavering edges | |
| Ramp | Gradient toward the stroke-start height, eased at both ends and with the 40 degree shoulder | Flip direction |
| Erode | Thermal weathering: slopes steeper than loose ground can rest at shed onto their lower neighbours, softening spikes into scree | |
| Rain | Hydraulic erosion: simulated rain under the brush carves gullies and drops sediment where it slows | |
| Pinch | Amplify deviation from the local average | Smooth toward it |
| Relax | Remove bumps and noise at a scale set by the brush, without shrinking the hills and valleys under them | |
| Retop | A wide, aggressive smooth | |
| Cliff | Layered rock strata: horizontal bands a few metres thick (scaled by the brush) that dip and undulate, hard bands standing as steep faces with ledges, soft ones weathering back to slopes | Soften |

**Stamp** is the seventeenth. Click rather than drag to stamp a shape once. Its card offers presets (Dome, Cone, Bell, Mesa, Ridge, Crater, Noise) under **Stamp**, a **Load PNG** button for a custom greyscale stamp, a blend mode, rotation in degrees and height in percent. Brush size sets the footprint.

### The brush cursor

Two rings, both riding the surface, so the cursor lies on a hillside rather than hovering flat above it.

The outer ring is the brush radius, drawn in the shape you picked. The inner ring is the edge of the full-strength core, so the gap between them is the falloff band. The colour says which brush is in hand.

### The brush card

| Setting | Range |
|---|---|
| Size | The brush radius, 1 to 200 metres. The scroll wheel resizes it while hovering the viewport. |
| Falloff | The curve by name (Smooth, Linear, Round, Sharp or Hard) and how much of the radius fades, 0 to 100% |
| Shape | Circle, Square or Diamond |

Under Falloff, a drawn curve shows the brush's real profile from its centre (left) to its rim (right), so you can see what a curve does before you try it.

Each setting shows its value: drag across it, or click to type one.

### The chosen brush's card

Every brush has **Strength** (1 to 100%). The rest belong to one brush each:

| Brush | Its settings |
|---|---|
| Flatten | **Affects**: Both, Raise (only fills hollows) or Lower (only cuts bumps). It levels to the height where the stroke starts |
| Set Height | **Target** in metres, and **Pick**: arm it, then click the ground to take the target from there (Escape cancels) |
| Noise | **Pattern** (Eroded, FBM, Ridge, Billow, Warped, Hybrid), **Feature size** in metres, **Detail** layers, **Roughness**, **Lacunarity**, **Warp** (Eroded and Warped only) and **Seed** |
| Terrace | **Step height** in metres and **Sharpness** (how short and steep each riser is; risers stay short of vertical) |
| Stamp | **Stamp** shape, **Load PNG**, **Blend**, **Rotation** in degrees, **Height** in percent |

The sculpt brushes have no slope or height mask; the paint brush does.

Smooth, Relax, Flatten, Set Height, Ramp, Terrace, Cliff and the erosion brushes all work across chunk seams, so nothing they do stops at a chunk edge. To weather the whole terrain at once, use **Erode all** in the Manage tab.

### Undo

Each stroke is one undo step, recorded on the main scene history, so it appears in the History panel alongside every other edit, named for the brush (`Sculpt: Flatten`).

## Heightmaps

The Manage tab's **Heightmap import and export** section has **Import...** and **Export...**, acting on the selected terrain.

Import accepts any PNG, 8 or 16 bit, in any colour layout, or a RAW16 file. It resamples across every chunk at full amplitude with no levelling, so a file that only uses part of its range imports as a correspondingly shallow terrain.

Export writes a 16-bit greyscale PNG of the whole composed heightmap.

Import is the blunt version. When you want a heightmap placed, levelled or blended with other shapes, use an imported-heightmap node in the terrain's [generator graph](/docs/r1-alpha8/editor/terrain-graph) instead.

## The landscape material

A terrain is textured by one node-graph material, made in the [Material Editor](materials.md) from the [Landscape nodes](../api/material-node-reference.md#landscape). The material says what each layer looks like and how layers mix; the terrain keeps how much of each layer every point has, as painted weights. Editing the material re-textures the terrain at once, and painting never changes the material.

Choose the material in the material slot at the top of the Terrain component: drop a `.material` on it, press **Choose...**, or press **Use Alpine Landscape** for the engine's default. **Edit** opens the material in the Material Editor. A terrain without one wears the blockout grid. The choice is saved with the terrain as its `TerrainMaterial` component.

### Alpine Landscape

The default material, `materials/landscape/alpine_landscape.material`. Pressing **Use Alpine Landscape** copies it, and its textures, into the project, so it ships with your game like any other material and you can open it in the Material Editor and change it.

| Layer | Texture | Tile | Blend | Rule |
|---|---|---|---|---|
| Grass | Grass, tinted summer green | 3 m | Height | The ground: everywhere nothing else is |
| Gravel | Dirt and pebbles | 2.5 m | Height | Slopes from 24 to 40 degrees |
| Rock | Riverbed rock, colour projected from the sides | 8 m | Height | Slopes over 34 degrees |
| Snow | Snow | 5 m | Weight | Above 190 metres, on slopes under 40 degrees |

The rules are worked out per pixel as the terrain draws, so they follow every sculpt stroke and generate: there is nothing to bake. Each rule's edge is broken up by noise, so a band ends in a ragged line rather than a contour, and a slow brightness drift crosses the land so distant hills are not one swatch.

Where two height-blended layers meet, their height maps decide which one shows, so grass settles into the cracks of a rock face and gravel gathers between stones instead of the two fading evenly into each other.

Rules and painting combine: a rule paints its layer on exactly as the brush would, over whatever was painted by hand. To paint rock where the rule would not put it, paint it; to keep rock off a cliff, lower the rule in the material.

The textures are CC0 from Poly Haven: 2048 pixel colour and normal maps, 1024 pixel packed and height maps, copied into the project under `textures/terrain/v2/`.

### Making your own

A landscape material is an ordinary Surface material. The usual shape:

1. One **Landscape Layer Coords** per layer, its Mapping Scale the metres one repeat of the textures covers. Its coordinates run across the whole terrain, so tiling never restarts at a chunk edge.
2. Sample each layer's textures with those coordinates. A normal map sampled through Layer Coords is oriented by the ground's own normal, which is what keeps lighting continuous across chunks.
3. One **Layer Blend** for the colour, one for the normal and one for the packed AO and roughness map, each with the same layer names, blend types, heights and rules, wired into the Surface Output.

The layer names on the Layer Blend nodes are what the terrain's Paint tab lists, in the order the material names them. A material can have up to eight layers, and sample up to sixteen different textures. Painted weights are matched to layers by name, so reordering or adding layers keeps what was painted; renaming a layer leaves its old paint unused until the name is used again.

### Older scenes

A scene saved with the old **Surface** component, or with painted overlay layers, is converted the first time it is opened in the editor:

- A Surface that was still the default becomes Alpine Landscape. One you had changed becomes a material of its own in `materials/landscape/`, named after the terrain, built from the same textures, tile sizes, tints and slope and height rules.
- Each painted layer becomes weight for the landscape layer it matches, by name first and then by what its material was called (a layer painted with a rock material becomes Rock). A layer painted without a material becomes Gravel, which is the dirt it drew.
- The old components are removed, so the next save holds only the new form. Save the scene to keep the conversion.

A game built from a scene that was never opened in the editor since still draws its Surface as before, but no longer draws overlay paint.

Under Solari, what the camera sees is the full material. Shadows, bounced light and reflections of the ground see a plain stand-in material, since a ray hit cannot run a node graph.

## Painting layers

The Paint tab paints the material's layers. The list is the material's layers, in the order its Layer Blend nodes name them, each with a swatch, its blend type and how much of the terrain it covers (updated when a stroke ends). Add a layer in the material graph and it appears here. Pick a layer and paint: the chosen layer rises and every other layer falls in proportion, so the layers at any point always add up to the whole. Unpainted ground is the material's first layer.

Picking a layer, or a mode, arms painting in the viewport. When the terrain has no material, a plain one, or one still loading, the list says so instead.

<!-- screenshot: terrain_paint.png - painting a landscape layer onto terrain, with the layer list visible -->

| Mode | Effect |
|---|---|
| Paint | Bring the layer toward the Target Weight, at the brush's strength |
| Erase | Take the layer away, giving its weight back to the others (or to the first layer, where there are none) |
| Smooth | Blur every layer together, softening edges |
| Fill | Lay the layer straight down at the Target Weight, with only the falloff to soften the edge |

Painting works across chunk seams, stays interactive on large terrains (only the texels under the brush change, and only they are sent to the GPU), and each stroke is one step on the undo stack.

To give the terrain another material, drop it on the material slot at the top.

### Paint brush settings

| Setting | What it does |
|---|---|
| Mode | Paint, Erase, Smooth or Fill |
| Size | Brush radius in metres, as for sculpting. The mouse wheel over the viewport scales it |
| Strength | How fast Paint, Erase and Smooth reach their result while the button is held, 1 to 100% |
| Target weight | The weight Paint and Fill bring the layer to. Below 100% leaves the layers under it showing through |
| Falloff, Shape | As for sculpting, with the same drawn curve; the same surface-following cursor shows both |
| Slope mask | When on, paint only where the ground's slope is between min and max degrees: rock only above 34 degrees |
| Height mask | When on, paint only where the ground is between min and max metres (world height): snow only up high |

Weights are stored per chunk at the chunk's own vertex spacing and saved with the scene in a compact encoded form, so an unpainted chunk costs a few bytes.

## Foliage

The **Foliage** tab paints a density map, and the runtime scatters animated grass blades into the painted areas, re-scattering as you sculpt underneath.

<!-- screenshot: foliage_paint.png - painting grass onto terrain with the foliage type tiles visible -->

The types are tiles: each shows the blade colours (tip over root), the name, and an estimate of how many blades it has on the terrain. Click a tile to paint with that type, which also arms foliage painting; the last tile adds a type. Eight types is the ceiling. Below the tiles, a card holds the chosen type's settings, with **Remove type** (the last type stays).

The **Brush** card: **Mode** (Paint, Erase, Grow, Trim), **Size** as a radius in metres, **Strength** and **Soft edge** in percent, and **Grow to** for Grow and Trim.

Grass appears as you drag, on every chunk the brush covers, so a stroke carries on across chunk seams. Every chunk it touched is rebuilt once more on release, so the result never ends on a stale preview.

Blades are 10 to 40 cm tall, so look from close to the ground to see them.

### Foliage type settings

| Setting | Range | What it does |
|---|---|---|
| Density | 1 to 128 per m² | Scatter clumps per square metre, default 48. This is the grid, not the blade count. |
| Blades / clump | 1 to 16 | Blades grown from each scatter point, default 5 |
| Height | 0.01 to 2 m | Minimum and maximum blade height |
| Width | 0.2 to 50 cm | Minimum and maximum blade width. It is not scaled by height. |
| Wind | 0 to 200% | How much the blades sway with the world wind |
| Enabled | On or off | Whether the type is drawn |

Blades per square metre is density times blades per clump, so 240 at the defaults.

Grass reads as grass when it comes in tufts. One blade per grid cell always leaves visible ground between cells, and a tuft is much cheaper than a finer grid because the whole clump shares one density lookup.

Painted weight sets coverage in proportion, with no floor under it, so half the paint scatters half the blades all the way down to bare ground. That is what keeps a patch inside the brush: the brush's own falloff leaves a weight gradient at the rim, which becomes a density gradient rather than a hard circle.

### Grow and Trim

Height Range sets how tall a type's blades are everywhere. **Grow** and **Trim** vary that across the ground, painting a height multiplier that every blade scattered there is scaled by. Unpainted ground is neutral.

Both aim at the brush card's **Grow to**, from 25% to 300% of the type's own height, approaching it at the brush's strength and through its soft edge. Set it to 100% and Trim returns grown ground to exactly neutral.

Grow only raises and Trim only lowers, so where two strokes overlap neither undoes the other.

Use it for a meadow that thickens into long grass in the hollows, a mown lawn with rough at the edges, or a worn path that stays cropped without going bald.

Two things to know:

- **The multiplier is per chunk, not per foliage type.** Height is a property of the ground, so a sheltered hollow grows everything in it taller.
- **It changes height, not coverage.** Trimming shortens blades without removing any. The density mask is still the tool for cutting a gap.

## Physics

Each chunk gets a triangle mesh collider. Rebuilding one is expensive, so it is debounced: it rebuilds shortly after the last mesh change and never mid-stroke. Sculpting stays responsive and the collider catches up on release.

## Scripting terrain

There is no terrain scripting API. Terrain is authored in the editor and saved into the scene. For runtime height queries, raycast against the mesh.

## Editing terrain from a plugin or MCP

The terrain editor publishes its operations through the contract crate as `renzora::terrain_tools::TerrainTools`, a resource of functions taking `&mut World`, so a plugin can edit terrain without linking the terrain crate:

| Function | What it does |
|---|---|
| `ground` | The terrain surface point and normal under a world `x`/`z` |
| `sculpt` | Drag any brush along a path of world points, with radius in metres. Raise, lower, sculpt and noise move the most-brushed ground by exactly the strength in metres; a ramp slopes from the ground at the first point to the ground at the last |
| `generate` | Run the generator over the terrain or a world rectangle |
| `erode` | Hydraulic and thermal erosion over the terrain or a disc |
| `build_graph` | Build a terrain graph into the terrain at full resolution, keeping its sculpting; optionally storing a new graph path and seed on its generator first (`TerrainGraphBuild`) |
| `set_material` | Give a terrain a landscape `.material`; an empty path is Alpine Landscape, copied into the project if needed |
| `layers` | The layers the terrain's material offers, with each one's blend type and coverage (`TerrainLayerInfo`) |
| `paint` | Paint one of those layers along a path of world points (`TerrainPaint`): paint, erase, smooth or fill, radius in metres, strength, target weight, falloff, and optional slope and height ranges |

Each edit is one step on the scene's undo stack. The MCP server's `terrain_height`, `place_on_ground`, `sculpt_terrain`, `generate_terrain`, `erode_terrain` and `scatter` tools are built on it.

The engine also ships a few CC0 rocks and ground plants for dressing terrain, as presets in `assets/models/nature/`: `list_assets` with `source engine` lists them, and `spawn_model` copies one into the project.
