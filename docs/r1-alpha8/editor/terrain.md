# Terrain

Generate, sculpt and paint heightmap terrain in the editor, then save it into your scene. Terrain renders in the editor and in your shipped game alike.

## Creating terrain

Add one from **+ Add Entity**, or click any terrain button in the viewport toolbar while no terrain exists.

A fresh terrain is a single 64 metre tile with a checkerboard placeholder material and a flat surface just above the editor grid. Its default height range is -10 to +40 metres.

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

Select the terrain root and the Inspector shows a **Terrain** section: a read-only size summary, an **Edit Terrain** button, and the live fields.

<!-- screenshot: terrain_inspector.png - the Terrain component in the Inspector with its sculpt brushes visible -->

**Min and Max Height are an envelope, not a height.** They define the band sculpting can reach, and heights are stored normalized inside it.

Two things follow. Generate's **Height** is capped by the range, not by Max Height. And changing either field **rescales the ground you already have**, because a vertex stored at 0.2 sits at a different world height once the envelope moves.

Set the envelope before you sculpt, rather than reaching for it later to make mountains taller.

To see how much of the band you are using, read the **Height** and **Range** rows of the [statistics overlay](/docs/r1-alpha8/editor/viewport#statistics). Height is the real relief, Range is the envelope it lives in.

### The Terrain Settings overlay

**Edit Terrain** opens everything that changes the terrain's structure. Nothing is written until you press **Apply**, so the expensive rebuild happens once for the size you actually chose.

| Setting | Range |
|---|---|
| Grid | Up to 32 chunks per axis, picked from a grid or typed |
| Chunk Size | 8 to 512 metres |
| Resolution | 33, 65, 129 or 257 vertices per side |
| Min and Max Height | The envelope above |
| Stream Chunks and Radius | Load chunks around the camera rather than all at once |

Resolution has fixed steps so neighbouring chunk edges share vertices exactly.

The structural fields are deliberately not live Inspector fields. A scrubbable field writes on every tick of a drag, and each write respawns every chunk, so dragging the grid from 1 to 8 would build every size in between.

## The tools

Four buttons appear in the viewport toolbar whenever a terrain exists.

| Button | What it does |
|---|---|
| Sculpt Terrain | Shape the ground |
| Paint Terrain Layers | Paint material coverage |
| Paint Foliage | Paint grass density |
| Resize Terrain | Drag the terrain's extent |

Clicking one selects the terrain and arms that tool. Click the active button again to go back to Select.

With a tool active, the brushes live on the **tool shelf** down the viewport's left edge. The active brush's settings sit in the **viewport toolbar**: size, strength, falloff, the shape toggles and the falloff curve, plus whatever that brush adds of its own.

The **Terrain Tools** panel is the secondary surface, carrying heightmap import and export, stamp loading, and the layer list. It and the toolbar drive the same state from either direction.

## Sculpting

Pick a brush from the shelf, then drag in the viewport.

| Brush | What it does | With `Shift` |
|---|---|---|
| Sculpt | Raise the terrain under the brush | Lower |
| Raise | Push up | |
| Lower | Push down | |
| Smooth | Average toward neighbours | |
| Flatten | Level toward the height where the stroke began | |
| Set H | Ease toward a target height | |
| Erase | Reset toward the flat baseline | |
| Noise | Add fractal noise | Smooth |
| Terrace | Snap heights to stepped plateaus | |
| Ramp | Gradient toward the stroke-start height | Flip direction |
| Erosion | Thermal erosion, lowering vertices steeper than the talus angle | |
| Hydro | Hydraulic erosion, sediment flowing downhill | |
| Pinch | Amplify deviation from the local average | Smooth toward it |
| Relax | Relaxation toward the neighbour average | |
| Retop | A wide, aggressive smooth | |
| Cliff | Steepen the local slope | Soften |

**Stamp** is the seventeenth. Click rather than drag to stamp a shape once. It offers presets (Dome, Cone, Bell, Mesa, Ridge, Crater, Noise), a **Load PNG** button for a custom greyscale stamp, a blend mode, rotation and height scale. Brush size sets the footprint.

### The brush cursor

Two rings, both riding the surface, so the cursor lies on a hillside rather than hovering flat above it.

The outer ring is the brush radius, drawn in the shape you picked. The inner ring is the edge of the full-strength core, so the gap between them is the falloff band. The colour says which brush is in hand.

### Brush settings

| Setting | Range |
|---|---|
| Size | 1 to 200 metres. The scroll wheel resizes it while hovering the viewport. |
| Strength | 0.01 to 1 |
| Falloff | 0 to 1, how far the soft edge reaches in from the rim |
| Shape | Circle, Square or Diamond |
| Falloff curve | Smooth, Linear, Spherical, Tip or Flat |

Some brushes add their own: Flatten has a mode and target height, Noise has scale, octaves and persistence, Terrace has steps and sharpness.

### Undo

Each stroke is one undo step, recorded on the main scene history, so it appears in the History panel alongside every other edit.

## Heightmaps

The Sculpt tab has **Import Heightmap** and **Export Heightmap**.

Import accepts any PNG, 8 or 16 bit, in any colour layout, or a RAW16 file. It resamples across every chunk at full amplitude with no levelling, so a file that only uses part of its range imports as a correspondingly shallow terrain.

Export writes a 16-bit greyscale PNG of the whole composed heightmap.

Import is the blunt version. When you want a heightmap *placed*, feathered, capped or blended onto ground you already have, use the **Generate** tool's heightmap source instead. For most real files that is the one you want.

## Painting layers

The **Paint Terrain Layers** tool paints coverage masks into a stack of up to 8 layers.

<!-- screenshot: terrain_paint.png - painting a material layer onto terrain, with the layer list visible -->

| Brush | Effect |
|---|---|
| Paint | Stamp the active layer's coverage. Overlapping strokes do not amplify. |
| Erase | Remove the active layer's coverage |
| Smooth | Blur the active layer's mask |
| Fill | Set coverage to full under the brush |

A fresh terrain starts with no layers. Click **Add Layer**, or just start painting, and the first stroke creates Layer 1.

Painting is non-destructive. Each layer keeps its own mask, and erasing or disabling one never touches the others.

### Layer settings

Drop a `.material` onto the active layer's slot to drive its appearance. The ✕ clears it, reverting to the placeholder green.

A layer wears the whole material, not an approximation, so a procedural graph with waves, noise or panning UVs renders on the terrain exactly as it does on a plane.

**Tile Size** is how much ground one repeat of the material covers, in metres, defaulting to 2. Set it to the real-world size of the thing the material depicts. Tiling in world units keeps a layer's scale fixed when the terrain is resized, and stays continuous across chunk seams.

**Height Offset** lifts the layer above the surface, and **Coverage Threshold** decides how much mask is enough to draw.

### Paint brush settings

Size here is a fraction of a chunk side rather than a metre count, from 0.01 to 0.5, so the brush scales with the terrain. Strength, falloff and shape work as they do for sculpting, and the same surface-following cursor shows all three.

## Foliage

**Paint Foliage** paints a density map, and the runtime scatters animated grass blades into the painted areas, re-scattering as you sculpt underneath.

<!-- screenshot: foliage_paint.png - painting grass onto terrain with the foliage type list visible -->

The shelf swaps to the foliage palette: **Paint**, **Erase**, **Grow** and **Trim**, then one numbered button per foliage type. Eight types is the ceiling.

Grass appears as you drag. Every chunk the stroke touched is rebuilt once more on release, so the result never ends on a stale preview.

### Foliage type settings

| Setting | Range | What it does |
|---|---|---|
| Density | 1 to 128 | Scatter clumps per square metre, default 48. This is the grid, not the blade count. |
| Blades per Clump | 1 to 16 | Blades grown from each scatter point, default 5 |
| Height Range | 0.01 to 2 | Minimum and maximum blade height, in metres |
| Width Range | 0.002 to 0.5 | The blade's actual width. It is not scaled by height. |
| Wind Strength | 0 to 2 | How much the blades sway |

Blades per square metre is density times blades per clump, so 240 at the defaults.

Grass reads as grass when it comes in tufts. One blade per grid cell always leaves visible ground between cells, and a tuft is much cheaper than a finer grid because the whole clump shares one density lookup.

Painted weight sets coverage in proportion, with no floor under it, so half the paint scatters half the blades all the way down to bare ground. That is what keeps a patch inside the brush: the brush's own falloff leaves a weight gradient at the rim, which becomes a density gradient rather than a hard circle.

### Grow and Trim

Height Range sets how tall a type's blades are everywhere. **Grow** and **Trim** vary that across the ground, painting a height multiplier that every blade scattered there is scaled by. Unpainted ground is neutral.

Both aim at the brush section's **Height** slider, from 0.25 to 3, approaching it at the brush's strength and through its falloff. Set the slider to 1 and Trim returns grown ground to exactly neutral.

Grow only raises and Trim only lowers, so where two strokes overlap neither undoes the other.

Use it for a meadow that thickens into long grass in the hollows, a mown lawn with rough at the edges, or a worn path that stays cropped without going bald.

Two things to know:

- **The multiplier is per chunk, not per foliage type.** Height is a property of the ground, so a sheltered hollow grows everything in it taller.
- **It changes height, not coverage.** Trimming shortens blades without removing any. The density mask is still the tool for cutting a gap.

## Physics

Each chunk gets a triangle mesh collider. Rebuilding one is expensive, so it is debounced: it rebuilds shortly after the last mesh change and never mid-stroke. Sculpting stays responsive and the collider catches up on release.

## Scripting terrain

There is no terrain scripting API. Terrain is authored in the editor and saved into the scene. For runtime height queries, raycast against the mesh.
