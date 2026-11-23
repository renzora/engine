# World Partition

Split a large scene into a grid of square cells that load and unload around the camera. A world partition lets you build and play worlds far bigger than your video memory: a whole city, a large open map, anything where keeping everything loaded at once is not an option.

Cells stream in the editor while you work, in Play, and in your shipped game.

## Partitioning a scene

1. In the Hierarchy, select what you want split into cells. Select a group to partition everything inside it, or select the objects themselves.
2. Right-click the selection and choose **Partition into cells**. Right-clicking a row that is not selected partitions just that row.

That's it. Every object moves into the cell its position falls in, and nothing moves in the world: each object keeps its exact position, rotation and scale. The scene gets a **World Partition** entity holding the settings, with one entity per cell (`Cell 3_-2`) filed under it, so the cells fold away in the Hierarchy like a folder. Each cell's contents are written to their own file beside the scene:

```text
scenes/main.bsn          the scene: settings and cell roots
scenes/main.cells/0_0.bsn
scenes/main.cells/3_-2.bsn
...
```

Run it again on more content at any time to add to an existing partition, including into cells that are not loaded right now (their files are extended, not replaced). Save the scene afterwards: the scene file now lists the cells.

A few things to know:

- **Models are placed by their bounds.** A model goes in the cell its bounding box centre falls in, read from the model file itself. So partitioning a city does not require loading the city first, and models whose real position is built into their geometry (city and GIS data usually arrives this way, every tile sitting at the origin) still land in the right cell.
- **Groups are kept by name.** A group is not moved itself; its contents are, and the group's name is recreated inside each cell that receives some of them. `Buildings/tile_12` stays `Buildings/tile_12` inside its cell, so a category is still easy to find and hide. Groups left empty are removed.
- **Cells live under the World Partition entity.** A scene partitioned before cells had a folder has its cells moved there when it opens, without any of them moving in the world; save to keep the tidier hierarchy. Leave the World Partition entity at the origin: the grid is measured from the world origin, so moving the folder would move every cell off its square.
- **Partitioning is not an undo step.** It writes the cell files straight away. Keep a copy of the scene if you want to experiment.

## How cells stream

A cell loads when the camera comes within the **Load Radius** of its centre and unloads beyond the **Unload Radius**, measured across the ground (X and Z only), so flying high over a city keeps the streets below loaded. Cells load in the background, nearest first, a few at a time, so moving the camera never stalls a frame. The viewport shows a progress card while cells are arriving.

A cell appears all at once, when it is ready: its contents stay hidden until every model in it has loaded, every mesh has its material and every texture is loaded, so a district never shows as black, untextured geometry while it arrives. While cells are arriving, texture and mesh uploads to the GPU are spread over several frames, so the frame rate holds instead of stalling for the few frames a dense block's textures would otherwise take. A cell whose textures never finish (a missing file) is shown anyway after 20 seconds.

When a cell unloads, its meshes, materials and textures are released, so memory follows what is near the camera rather than the size of the world.

## Distant LODs: nothing disappears

Streaming alone empties the world beyond the unload radius, and from any height that means districts vanishing behind the camera. **Build distant LODs**, in the World Partition entity's Inspector section, gives every cell a stand-in: its whole contents merged into one simple mesh, coloured from each building's own textures, saved beside the cell as `x_z.hlod.glb`.

A stand-in is shown whenever its cell's full detail is not on screen, and hidden the moment the full cell appears (which itself waits until the cell is fully loaded), so the swap leaves no gap. Stand-ins are small and always loaded: the skyline is complete from any distance and any direction, in the editor and in the game.

- **Building them** reads every cell's file and models from disk, so cells do not have to be loaded. It runs in the background with a progress card in the viewport; a large city takes a minute or two.
- **Rebuild after editing cells.** A stand-in shows the cell as it was when built. Building again replaces them all.
- **They are vertex-coloured.** Each wall gets the average colour of its texture, which reads correctly at a distance but carries no facade detail. Up close you see the full cell anyway.
- **They are not saved in the scene.** The scene only records which cells have one; the stand-ins themselves are files, spawned when the scene opens.
- **Gaps are reported.** If a cell's full detail goes out of view with nothing drawable in its place, the Console says so and why: the cell has no stand-in, or its stand-in has no mesh yet (still loading, or empty).

## The World Partition section

Everything about the partition is on the **World Partition** entity: select it in the Hierarchy and its section in the Inspector holds the settings, the map and the tools.

| Field | Meaning |
|---|---|
| Cell Size | Width of one square cell, in metres. Default 1000 |
| Load Radius | Ground distance from the camera to a cell's centre at which it loads. Default 1500 |
| Unload Radius | Ground distance at which a loaded cell unloads. Default 2000. Always kept a little above Load Radius so a cell on the boundary can't flicker |
| Stream in editor | Turn it off to keep every cell loaded while editing, the way ordinary scene instances behave. Cells still stream in Play and in the game |
| Load all cells | Load the whole map in the editor, for the times you need to see all of it. A big world needs the memory to match. Ignored in Play and in the game |

Below the settings:

- **Readouts.** How many cells there are, how many are loaded, loading and pinned, how many have a distant LOD, and which cell the camera is in.
- **Map.** Every cell seen from above: loaded, loading or unloaded, with the camera's cell framed.
- **Pinning.** Click a cell on the map to pin it. A pinned cell stays loaded wherever the camera goes, for when you are working on one area and want to look around. **Unpin all** releases them. Pins are yours, not the scene's: they are not saved.
- **Build distant LODs.** See [Distant LODs](#distant-lods-nothing-disappears).

The map and readouts update only while the section is open, so a collapsed section costs nothing.

Change the cell size before partitioning. Changing it afterwards does not regroup existing cells: partition again into a fresh scene instead.

## Editing inside cells

Edit anything in a loaded cell as usual. Your edits are kept:

- **Saving** the scene writes every loaded cell back to its file.
- **A cell streaming out** while the scene has unsaved changes is written back first.
- **Switching scene tabs** writes loaded cells back before the scene is put away.

A cell's contents are part of the scene from then on: discarding unsaved changes to the scene afterwards does not undo what a cell already wrote to its file.

## Limits

- **Textures stream separately.** Texture streaming (full and low-resolution textures by distance) runs in Play and the game, not in the editor. See [Streaming](../engine-core/streaming.md#texture-streaming).
- **Large hierarchies cost the editor.** While cells arrive, the Hierarchy panel rebuilds its tree, which on a world with tens of thousands of entities shows up as frame time while cells are loading.
- **Coordinates far from the origin lose precision.** Positions are single precision, which is fine to millimetres tens of kilometres out but not across a continent. Keep a world within about 50 km of the origin.
