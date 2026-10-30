# Terrain Graphs

A terrain graph is a recipe for a landscape: a node graph that builds a heightfield from primitives (noise, mountain ranges, dunes, craters, an imported heightmap), shapes it (combine, curves, terraces, blur), lets nature add the detail (erosion, rivers, lakes, snow) and measures masks from it (slope, height, flow) for paint layers and foliage. It is a `.tgraph` file, so one graph can generate any number of terrains, each with its own seed.

Each node shows a shaded preview of its result, the graph re-evaluates in the background as you edit, and **Build** writes the result into the terrain's base heights. Hand sculpting lives on a separate layer above them, so rebuilding never throws it away.

## Making a graph

In the Assets panel, open the create menu and pick one of:

| Entry | What it makes |
|---|---|
| Terrain Graph: Highlands | Eroded mountain ranges over meadow foothills. Masks: Gravel (where water runs), Dirt (where it drops sediment), Meadow grass (low, gentle, southern foothills) |
| Terrain Graph: Rolling Hills | Soft hills with gentle valleys. Masks: Grass, Dirt, Rock |
| Terrain Graph: Desert | Dune fields around terraced mesas. Masks: Sand, Rock |
| Terrain Graph | One Eroded noise into the output: a starting point |

Double-click a `.tgraph` to open it in the **Terrain** workspace: the graph on the left, the viewport and the **Terrain Graph Inspector** on the right.

## Using a graph on a terrain

Select the terrain and open the **Manage** tab of its Terrain component. The **Generator** card holds the terrain's `TerrainGenerator`:

| Control | What it does |
|---|---|
| Graph slot | The `.tgraph` to build from: drop one from the Assets panel or press **Choose...**. Shows the graph's name, node count and when it was last built |
| Edit | Open the graph in the Terrain workspace |
| Seed, Re-roll | The landscape's seed. The same graph with another seed is the same kind of landscape, a different one |
| Exposed parameters | One row per parameter the graph exposes (such as Highlands' peak height), overridden for this terrain |
| Build, Cancel | Build in the background at the terrain's full resolution, with a progress bar and the last build's outcome |

**Build** in the graph editor's toolbar (or its inspector) builds the open graph into the terrain it targets: one whose generator already names this graph, else the selected terrain, else the only one. A terrain that named another graph, or none, is pointed at this one.

## The editor

- **Add a node** with Add Node, by right-clicking the canvas, or by dragging a cable from a pin onto empty canvas (the new node is wired to it). The palette groups nodes by family.
- **Connect** by dragging from an output pin to an input. An input takes one wire. Wires that would make a loop are refused.
- **Delete** the selected nodes with Delete. **C** puts the selected nodes in a comment box.
- **Parameters** of the selected node are in the Terrain Graph Inspector. The pin icon beside a number **exposes** it: each terrain using the graph can then override it. Exposed parameters are listed under the node parameters.
- **Previews** are evaluated at a quarter of the target terrain's resolution (between 33 and 129 samples across), over the same extent, with its seed and overrides. Parameters are in metres, not samples, so the preview looks like the build. A height previews as shaded relief, lit from the north-west, low ground slate to high ground white; a mask previews as grey, black 0 to white 1. North is up.
- **Errors** show on their node (a red mark, the message on hover) and in a strip under the toolbar.
- **Save** with Save or Ctrl+S. Build uses the open graph as it is, unsaved edits included, so what you see in the previews is what builds.
- Every edit is undoable with Ctrl+Z while the graph is the active document.

## Units

Heights and distances are **metres**. Angles are **degrees**. Bearings are compass bearings: 0 is north (+Z), 90 east (+X). Masks are **0 to 1**. Offsets are from the terrain's centre. Nothing is in grid cells, which is what keeps a preview and a build alike.

Pins are **Height** (amber) or **Mask** (blue). Either connects to either: a mask is a height of 0 to 1 m.

Every node with randomness also has a **Seed** parameter, added to the terrain's seed for that node only, so two copies of a node can differ.

## Nodes

### Primitives

| Node | Parameters | What it makes |
|---|---|---|
| Noise | Mode (Eroded, FBM, Ridged, Billow, Warped, Hybrid), Scale m, Height m, Octaves, Lacunarity, Persistence, Warp, Exponent, Offset X/Z m, Seed | Fractal noise, in every mode of the sculpt Noise brush. Exponent above 1 sharpens peaks and widens valleys |
| Mountain Range | Scale m, Height m, Direction deg, Elongation, Warp, Octaves, Sharpness, Seed | Ridged, warped ranges running along a bearing, rising and falling along their length |
| Plateau | Offset X/Z m, Radius m, Height m, Edge m, Edge Noise, Noise Scale m, Seed | A flat-topped mesa with a ragged outline |
| Crater | Offset X/Z m, Radius m, Depth m, Rim Height m, Rim Width m | A bowl below zero with a raised rim |
| Volcano | Offset X/Z m, Radius m, Height m, Crater Radius m, Crater Depth m, Flank Curve, Roughness, Seed | A cone with concave flanks and a summit crater |
| Dunes | Wavelength m, Height m, Wind deg, Asymmetry, Meander, Seed | Dune ridges: a long windward slope and a short steep lee face |
| Cells | Mode (Distance, Ridges, Flat Cells), Scale m, Height m, Jitter, Seed | Voronoi cells: pits, cracked walls, or steps |
| Gradient | Mode (Linear, Radial), Direction deg, Start, End | A ramp across the terrain as a mask, edge to edge along a bearing or from the centre out |
| Constant | Height m | One height everywhere |
| Import Heightmap | File, Height m, Normalize, Invert | A PNG (8 or 16 bit grey) or RAW16 file stretched over the terrain, white at Height. Normalize stretches the file's own value range to the full height |
| Current Terrain | | The terrain's generator layer as it is now (its last build, or its ground before the first one) |

### Shape

| Node | Inputs | Parameters | What it does |
|---|---|---|---|
| Combine | A, B, Mask | Mode (Add, Subtract, Max, Min, Multiply, Blend, Difference), Ratio | Combines A and B. Ratio times Mask fades from A (0) to the full result (1). Blend takes B |
| Curve | Height | Points | Remaps through a curve over the input's own range (x is the input, y the output, both 0 to 1). Smooth and never overshoots between points. Points are typed as `x,y` pairs |
| Terrace | Height | Step Height m, Sharpness, Offset m | Flat treads joined by risers. Sharpness 0 is a ramp, 1 a short steep riser |
| Warp | Height, Warp X, Warp Z | Strength m, Noise Scale m, Seed | Pushes the input sideways, by the two masks (0.5 is no push) or by noise when neither is wired |
| Blur | Height | Radius m | Softens |
| Sharpen | Height | Radius m, Amount | Boosts detail finer than the radius |
| Transform | Height | Scale, Rotate deg, Offset X/Z m, Height Scale, Height Offset m | Scales, turns and moves the input across the ground, about the centre, and scales or lifts its heights |
| Clamp | Height | Min m, Max m | Cuts everything off outside the range |
| Invert | Height | Mode (Within Its Range, Mask) | Upside down, keeping the lowest and highest heights where they were; Mask is 1 minus the input |
| Height Range | Height | Min m, Max m | Stretches the input so its lowest point is Min and its highest Max |
| Flatten | Height, Mask | Height m, Strength | Pulls toward one height where the mask is set: plots, roads, lake beds |

### Simulate

| Node | Outputs | Parameters | What it does |
|---|---|---|---|
| Erosion | Height, Flow, Deposits | Rain, Erosion, Deposition, Capacity, Radius m, Weathering, Repose deg, Seed | Raindrops carve gullies and drop sediment in the valleys, then slopes steeper than the repose angle shed scree. Rain is raindrops per 3 m cell of ground, so a preview and a build rain alike. The Mask input fades the result back to the input. Flow is where water runs; Deposits where sediment landed. The same erosion as the Erosion brush |
| Thermal | Height, Talus | Passes, Repose deg | Weathering alone. Talus is where the scree settled |
| Rivers | Height, Water | Catchment m², Depth m, Width m | Carves channels where the uphill area draining through a point passes Catchment |
| Lakes | Height, Water | Min Depth m, Fill | Fills every basin to the level it would spill at. Fill raises the ground to the water (flat lake beds); otherwise only the Water mask changes |
| Snow | Height, Snow | Snow Line m, Blend m, Max Slope deg, Depth m | Snow on high, gentle ground, Depth metres of it added to the height |

### Data

Every data node outputs a 0 to 1 mask.

| Node | Parameters | What it measures |
|---|---|---|
| Slope | Min deg, Max deg, Falloff deg | Ground whose steepness is in the range |
| Height | Min m, Max m, Falloff m | Ground between two heights |
| Curvature | Mode (Convex, Concave, Both), Radius m, Range m | Ridges or hollows over a radius; Both is grey on flat ground |
| Flow | Min Area m², Max Area m² | Uphill area draining through each point, on a log scale between the two areas |
| Wetness | Min Area m², Max Area m² | Gathered water, hollows and flat ground, combined |
| Aspect | Facing deg, Spread deg | Slopes facing a bearing (180 is south) |
| Select Range | Low, High, Falloff | A band of the input's own range, 0 its lowest and 1 its highest |

### Output

| Node | Parameters | What it does |
|---|---|---|
| Terrain Output | Range (Fit Floor to Peak, Absolute), Floor m, Peak m, Offset m | The terrain's generator layer. Fit stretches the input from Floor to Peak; Absolute takes its metres plus Offset. A graph has one; only the first is built |
| Mask Output | Name | A named mask in the build result, for paint layers and foliage. Its name is in its title |

## Layers: generator and sculpt

A terrain built from a graph has two layers:

- The **generator layer** is what the last build wrote. It is stored per chunk (`TerrainGeneratorLayer`, in metres) and saved with the scene.
- The **sculpt layer** is everything done to the ground since: brushes, the Erode button, heightmap import, MCP edits. It is the difference between the ground and the generator layer.

A rebuild replaces the generator layer and puts the sculpt layer back on top of it. A hill you raised by 3 m is 3 m above the new ground wherever the new ground is. Brushes, erosion, undo and the MCP tools work exactly as they did; they edit the ground, and the layer split is kept beside it.

Some things to know:

- The **first build** of a terrain (or of chunks respawned by a resolution change in the Manage tab) has no generator layer to compare with, so it replaces the ground. Undo brings it back.
- If a build reaches above Max Height or below Min Height, the terrain's height range **grows** to hold it. Nothing already there moves.
- A build is **one undo step** on the scene's history, covering the heights, the generator layer and any range change.
- The masks of the last build are on the terrain as `TerrainGraphOutput` (not saved; a rebuild recreates them).

## Builds

A build gathers the graph, the terrain's whole vertex grid (chunks share their edge vertices, so seams always match) and its current generator layer, evaluates on a background thread, and lands on the main thread. Starting a second build of the same terrain cancels the first. Sculpting while a build runs is fine: the sculpt layer is read when the build lands.

Evaluation is deterministic: the same graph, seed and overrides give the same terrain, whatever the machine's core count. Nodes cache their results, so an edit re-runs only that node and what it feeds.

## File format

A `.tgraph` is JSON:

```json
{
  "version": 1,
  "name": "Highlands",
  "nodes": [
    { "id": 1, "type": "mountain_range", "position": [0, 0], "params": { "scale": 800.0, "height": 260.0 } },
    { "id": 5, "type": "terrain_output", "position": [860, 40], "params": { "peak": 290.0 } }
  ],
  "connections": [
    { "from_node": 1, "from_pin": "height", "to_node": 5, "to_pin": "height" }
  ],
  "params": [
    { "name": "peak_height", "label": "Peak height", "node": 5, "param": "peak", "min": 10, "max": 3000 }
  ]
}
```

A node stores only the parameters that differ from their defaults. `params` lists the exposed parameters. Node type ids are the node names in snake case (`noise`, `mountain_range`, `height_mask` for the Height data node, `terrain_output`, `mask_output`).

## For plugins and MCP

- `renzora::terrain_tools::TerrainTools::build_graph` builds a graph into a terrain synchronously, taking a `TerrainGraphBuild` (terrain, optional graph path, optional seed). See [Terrain](/docs/r1-alpha8/editor/terrain#editing-terrain-from-a-plugin-or-mcp).
- In the editor, writing a `renzora_terrain_graph::BuildTerrainGraph { terrain }` message starts a background build; `TerrainGraphBuilds` reports its progress and outcome per terrain.
- `renzora_terrain_graph::exposed_params(graph, generator)` lists what a graph lets a terrain override, with each value.
