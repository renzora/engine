# Tilemaps

Paint 2D tile maps from tileset atlas images. The **Tilemap** panel owns the whole workflow: importing tilesets, switching between the scene's tilemaps, and picking the brush you paint with.

<!-- screenshot: tilemap_panel.png - the Tilemap panel with a tileset atlas, the layer list and the tab strip -->

Every painted tile is a real sprite entity. Tiles show in the Hierarchy, and they select, move, save, load and animate exactly like hand-placed sprites.

## Importing a tileset

Drag a tileset image from the **Assets** panel onto the Tilemap panel. A "Drop tileset to import" overlay confirms the target, and releasing creates one tilemap per dropped image, named after the file.

Accepted formats: `png`, `jpg`, `jpeg`, `webp`, `ktx2`, `rmip`.

Tileset atlases are pinned to nearest filtering, even if another system loaded the same image with linear filtering first, so pixel art stays crisp and tile seams never bleed neighbouring cells.

Dropping a tileset that is already imported just switches the panel to it.

There is no Add Entity preset for tilemaps. The panel is the one import surface.

## Multiple tilemaps

A tab strip at the top lists every tilemap in the scene. Click a tab to make it active, and the palette below shows its atlas. Click the active tab again to deselect it, which also drops the brush.

Selecting a tilemap in the Hierarchy activates it too, but you do not need to keep it selected while painting.

Each tilemap is an ordinary entity. Rename it, move its Transform to offset the whole map, toggle its visibility, or delete it.

Per-tilemap settings live on the **Tilemap Layer** component: **Tile Size**, **Atlas Tile Px** and **Columns**. The defaults are 16 pixel atlas cells at 16 world units per tile.

## Paint layers

Under the tab strip is the layer list, with the top row drawing on top. Every tilemap starts with just **Base**. Click **Add Layer** to stack more.

- **Click a row** to make it the paint target. Strokes only touch the selected layer, so decoration on an upper layer never eats the ground under it.
- The **eye** toggles visibility, and the **lock** protects a layer from painting and erasing.
- The **carets** nudge a layer's draw order. An overhead layer above the base draws over everything, including y-sorted props, which sort only within their own layer.

Layers are ordinary child entities of the tilemap. They share its tileset and settings, and each keeps its own tiles.

## Picking a brush

The panel shows the active tilemap's atlas in a zoomable, pannable view.

| Action | How |
|---|---|
| Select a tile | Left-click |
| Select a block | Left-drag. Drag the corner handle to grow it. |
| Build a non-rectangular pick | `Ctrl+Click` toggles a cell, `Shift+Click` adds one |
| Zoom | Wheel, or the `-` and `+` buttons |
| Pan | Right-drag |
| Show tile boundaries | The **Grid** switch |

A non-rectangular pick is how you grab a tree's canopy branches beside its narrow trunk without dragging over the bushes between them. Each individually picked cell is tinted so you can see what is selected.

Selecting tiles arms the brush by switching the viewport's **Mode** dropdown to **Paint**. `Tab` toggles Select and Paint while the pointer is over the 2D viewport, and `Esc` drops back to Select.

## Painting

In Paint mode a semi-transparent ghost of the selected block follows the cursor, snapped to the grid, showing exactly what a click will stamp and where.

<!-- screenshot: tilemap_paint.png - the 2D viewport mid-paint with the brush ghost following the cursor -->

| Action | What it does |
|---|---|
| Left-drag | Paint. Interpolated, so a fast drag never skips cells. |
| `Shift` and drag | Fill a rectangle. The press anchors a corner and the ghost previews the fill. |
| `Alt` and left-drag | Erase |
| `Alt` at the press of a `Shift` drag | Rectangle-erase. The region ghosts red. |
| Right-click | Drop the brush and return to Select |
| `Esc` or `Tab` | The same |

Re-painting a cell replaces its tile rather than stacking a second one.

Erasing is deliberately not on right-drag, which stays free for the 2D camera pan. Only a right-click with no movement switches the mode.

There is also a dedicated **Erase** mode in the Mode dropdown, where every left-drag erases and `Shift` and drag rectangle-erases without needing `Alt`.

While Paint mode is on, the 2D selection tools stand down, so painting never accidentally moves a sprite. Painting is edit-mode only.

## Multi-tile objects

Select more than one tile in the palette and painting stamps a **single composite entity**. A tree or a house is one sprite, not a loose pile of cells.

Because it is a single entity, a multi-tile object:

- selects, moves, rotates and scales as one;
- erases as one, so erasing anywhere on it removes the whole object;
- saves and ships as one.

A single-cell selection still paints ordinary per-cell tiles, so normal tilemapping is unchanged.

## Randomise

Filling an area with a single tree gives you an obviously tiled grid. **Randomise**, the dice button in the viewport toolbar, turns that into a natural scatter. It is the fast way to lay down a forest, a field of rocks or scattered foliage.

The workflow is select, then randomise, so it works on tiles you have already placed.

1. Paint or rectangle-fill a block of trees. A plain solid grid is fine.
2. Switch to **Select** and select them, by rubber-band drag or `Ctrl+Click`.
3. Click the dice.

Each selected tile moves to a random cell within the selection's own bounding box. There are no knobs. The first press opens a solid block into an uneven scatter with gaps and clusters, and nothing spills outside the area you selected.

Click again for a completely different layout. Repeat presses reshuffle the same tiles within the same area rather than thinning the field further, so you can keep clicking until you like it.

Only real painted tiles are touched, so anything else in a mixed selection is left alone.

## Tile collision

Mark tiles as solid in the palette and every painted copy of them collides. Walls, cliffs, water edges, without placing a single collider by hand.

1. Select the solid tiles in the palette.
2. Click the **wall** button in the panel header. The marked cells tint red.

Clicking it again with the same cells selected unmarks them.

That is the whole authoring step. The engine grows merged static colliders under the layer: contiguous solid tiles are merged into rectangles, so a 20-tile wall is one collider rather than twenty, and moving bodies never snag on seams.

Colliders regenerate whenever you paint, erase or change the solid set, and on every scene load and in the exported game, so nothing extra is saved.

The solid set lives on the Tilemap Layer component, so it applies to every tile of that layer, past and future. Mark a wall once and every wall you ever paint is solid.

### Object collision

Multi-tile objects are not covered by the solid set. They get a collision shape, authored in the palette.

1. Pick the object in the palette.
2. Click the **wall** button. A green collision box appears over the selection, covering the whole footprint.
3. Drag its handles to resize, or drag inside it to move. Shrink it down to the trunk base.
4. Pick a **shape** in the dropdown beside the wall button: Box, Circle or Capsule.
5. Paint. Every stamped object carries that shape as a real Collision Shape component.

The box is remembered per palette region and saved with the scene, so re-picking the same tree later recalls its collider and every future stamp gets it.

Objects painted before the box existed keep the collider they were stamped with. Reshape those individually with the Inspector's collider **Edit** toggle, or re-stamp them.

Stamped objects also come with **Y Sort** already on, pivoting at their bottom edge, so a character with Y Sort at their feet walks behind the canopy and in front of the trunk with no extra setup. See [The 2D View](/docs/r1-alpha8/editor/2d-view#y-sorting).

## Selecting painted tiles

Switch the Mode dropdown back to **Select** and the normal 2D picking tools return.

`Ctrl+Click` toggles individual tiles in and out of a multi-selection, which is how you gather scattered tiles to move or delete together. `Shift+Click` adds without toggling. Dragging a rubber band over empty space box-selects.

Selection is pixel-perfect, so clicking a fully transparent part of a tile falls through to whatever is behind it.

## How tiles are saved

Every painted tile is a named child of the tilemap, `Tile (x, y)` for a single cell and `Object (x, y)` for a composite, saved with the scene like any sprite.

The tileset is stored as a project-relative path, so projects stay portable across machines.
