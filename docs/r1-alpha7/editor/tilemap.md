# Tilemaps

Paint 2D tile-based maps from tileset atlas images. The **Tilemap** panel (2D panel group, icon "grid-four") owns the whole workflow: importing tilesets, switching between the scene's tilemaps, and picking the brush you paint with in the 2D viewport.

## How it works

Tilemaps are two crates working together:

- **`renzora_tilemap`** — the runtime data types. `TilemapLayer` is the authored, scene-saved palette config: the tileset's asset path, the tile size, and how the atlas is sliced. **Every painted tile is a real sprite entity** — a child of the layer entity carrying `TilemapTile` (its grid cell) plus the engine's standard persisted sprite components (`SpriteImagePath`, `SpriteSheet`, `SpriteCustomSize`) — so tiles show in the hierarchy, and they select, move, save, load and animate exactly like hand-placed sprites in both the editor and your shipped game. A **multi-tile brush** paints a single composite sprite instead of one-per-cell (see [Multi-tile objects](#multi-tile-objects)), using a `SpriteAtlasRegion` component to store the atlas slice. Registered with `renzora::add!(TilemapPlugin)`.
- **`renzora_tilemap_editor`** — the editor-only half: the Tilemap panel, the paint brush, the cursor ghost preview, and tileset importing.

## Importing tilesets

**Drag a tileset image from the asset browser onto the Tilemap panel.** While the drag hovers the panel a highlighted **"Drop tileset to import"** overlay confirms the target; releasing creates one tilemap per dropped image — a `TilemapLayer` entity named after the file (a multi-select drag imports every image in it). Accepted formats: `png`, `jpg`, `jpeg`, `webp`, `ktx2`, `rmip`.

Tileset atlases are pinned to **nearest filtering** — even if another system (a thumbnail, a plain sprite) loaded the same image with linear filtering first — so pixel art stays crisp and tile seams never bleed neighbouring atlas cells.

Dropping a tileset that's already imported doesn't duplicate anything — it just switches the panel to that tilemap.

There is no Add-Entity preset for tilemaps; the panel is the one import surface.

## Multiple tilemaps

A **tab strip** at the top of the panel lists every tilemap in the scene. Click a tab to make that tilemap the **active** one — the palette below shows its atlas, and painting writes into it. Click the active tab again to **deselect** it (this also drops the brush, returning the viewport to normal picking). Selecting a tilemap entity in the hierarchy also activates it, but you do **not** need to keep it selected while painting.

Each tilemap is an ordinary scene entity: rename it in the hierarchy, move its `Transform` to offset the whole map (tiles are its children and follow), toggle its `Visibility`, or delete it to remove the map. Per-tilemap settings (**Tile Size**, **Atlas Tile Px**, **Columns**) live on the `TilemapLayer` component in the inspector — defaults are 16 px atlas cells at 16 world units per tile, matching the sprite convention of 1 px = 1 world unit.

## Paint layers

Under the tab strip sits the **layer list** — Godot-style, top row draws on top. Every tilemap starts with just **Base** (the tilemap itself); click **Add Layer** to stack more:

- **Click a row** to make it the paint target — strokes (paint, rectangle fill, erase) only touch the selected layer, so decoration on an upper layer never eats the ground under it.
- The **eye** toggles a layer's visibility, and the **lock** protects it from painting and erasing entirely.
- The **carets** nudge a layer's draw order up or down. An "Overhead" layer above the Base draws over everything on it — including y-sorted props, which sort only within their own layer.

Layers are ordinary child entities of the tilemap (rename them in the hierarchy); they share the tilemap's tileset and settings, and each keeps its own tiles. Solid-tile collision works per layer — marking "wall" solid applies to walls on any layer. Scenes made before layers existed keep working: their tiles simply live on Base.

## Picking a brush

The panel shows the active tilemap's atlas in a zoomable, pannable view:

- **Left-click** a tile to select it; **left-drag** to select a rectangular block. The orange highlight shows the selection, and its corner handle can be dragged to grow the block.
- **Ctrl + click** toggles an individual tile in or out of the selection, and **Shift + click** adds one — so you can build a **non-rectangular** pick by clicking (grab a tree's canopy branches beside its narrow trunk without dragging over the bushes/dirt between them). Each individually-picked cell is tinted so you can see exactly what's selected.
- **Wheel** zooms toward the cursor; **right-drag** pans. The `−` / `+` buttons and the zoom dropdown in the header do the same.
- The **Grid** switch overlays tile boundaries on the atlas.

Selecting tiles **arms the brush** by switching the viewport's **Mode** dropdown to **Paint** — the dropdown (next to the 3D/2D/UI selector) is the single source of truth for the mode. **Tab** toggles Select ↔ Paint while the pointer is over the 2D viewport, and **Esc** drops back to Select.

## Painting

In Paint mode, move the cursor over the 2D viewport — a **semi-transparent ghost of the selected block** follows it, snapped to the active tilemap's grid, showing exactly what a click will stamp and where.

- **Left-drag** paints. A **single (1×1) brush** paints one tile per cell, interpolated so a fast drag never skips cells, and re-painting a cell replaces its tile instead of stacking a second one. A **multi-tile brush** paints [composite objects](#multi-tile-objects) — one object per position, tiled edge-to-edge as you drag.
- **Shift + drag** fills a **rectangle**: the press anchors a corner, the drag sizes the region (the ghost previews the fill), and release fills it — a 1×1 brush tiles single tiles, a multi-tile brush tiles whole objects on a block-sized lattice. Hold **Alt** at the press to rectangle-*erase* instead (the region ghosts red).
- **Alt + left-drag** erases. (Erasing is *not* on right-drag — that stays free for the 2D camera pan.)
- **Right-click** (a click, not a drag) drops the brush and returns to Select — a quick way out of Paint/Erase without reaching for Esc/Tab. A right-*drag* still pans the 2D camera, so only a click with no movement switches the mode.
- **Esc** / **Tab** (or picking **Select** in the Mode dropdown) drops the brush and returns the viewport to normal picking. Deselecting the tilemap in the tab strip does the same.

## Multi-tile objects

Selecting **more than one tile** in the palette and painting stamps a **single composite entity** — a tree, house, or any multi-cell prop is *one* sprite, not a loose pile of cells. Each click (or each edge-to-edge step of a drag) creates one such object; the block keeps the orientation it has in the palette.

Because it's a single entity, a multi-tile object:

- **selects, moves, rotates and scales as one** — click it in Select mode and the whole thing is the selection;
- **erases as one** — erasing anywhere on it removes the entire object, not just the cell under the cursor;
- **saves and ships as one** — it reopens and exports identically.

How it's stored depends on the shape of the pick:

- A **solid rectangle** is one sprite cropped to that atlas region (a `SpriteAtlasRegion`), sharing the tileset texture — the cheap path.
- A **non-rectangular** pick (built with Ctrl/Shift+click) can't just crop the atlas — the bounding box would drag in the neighbouring tiles. Instead the picked cells are **baked into a fresh texture** (a `TileObject`), with the unpicked cells left transparent, so only the tiles you chose are drawn. The bake is regenerated on load and in the exported game, so nothing extra is stored on disk.

A **single (1×1)** selection still paints ordinary per-cell tiles, so normal tilemapping is unchanged — the composite behaviour only kicks in once you pick more than one cell.

## Randomise (make a forest)

Filling an area with a single tree gives you a rigid, obviously-tiled grid. The **Randomise** button (the dice icon) in the **viewport toolbar** turns that grid into a natural-looking scatter — the fast way to lay down a forest, a field of rocks, or scattered foliage. It only appears in 2D view once you have painted tiles selected.

The workflow is **select-then-randomise**, so it works on tiles you've already placed:

1. Paint (or rectangle-fill) a block of trees — a plain solid grid is fine.
2. Switch to **Select** and select them — rubber-band drag over the block, or Ctrl/Shift+click to gather them.
3. Click the **dice** button in the viewport toolbar.

Each selected tile is moved to a random cell **within the selection's own bounding box**. There are no knobs to fiddle: on the **first** press a solid block naturally opens up into an uneven scatter (roughly 60–65% coverage) with gaps and clusters — the "forest" look — while the trees never spill outside the area you selected.

Randomise is **repeatable** — click it again for a completely different layout. Repeat presses reshuffle the *same* trees within the *same* area: they don't thin the field further or pull it inward, so you can keep clicking until you like the arrangement. (Changing the selection starts a fresh pass over the new bounds.) Only real painted tiles are touched, so anything else in a mixed selection is left alone. Because each tile keeps a real grid cell (its `TilemapTile` moves with it), erasing and re-painting those cells still work normally.

## Tile collision

Mark tiles as **solid** in the palette and every painted copy of them collides — walls, cliffs, water edges — without placing a single collider by hand:

1. Select the solid tiles in the palette (a wall block, the cliff edges — any selection works, including Ctrl/Shift picks).
2. Click the **wall** button in the panel header. The marked cells tint **red** in the palette. Clicking it again with the same cells selected unmarks them.

That's the whole authoring step. Behind the scenes the engine watches the layer's painted tiles and grows **merged static 2D colliders** under it: contiguous solid tiles are greedy-merged into rectangles (a 20-tile wall is *one* collider, not twenty), so the physics world stays small and moving bodies never snag on seams between tiles. The colliders regenerate whenever you paint, erase, or change the solid set — and on every scene load and in the exported game, so nothing extra is saved in the scene file.

The solid set lives on the `TilemapLayer` component (`solid_tiles`, a list of atlas cell indices), so it saves with the scene and applies to every tile of that layer, past and future — mark "wall" once, and every wall you ever paint is solid.

Colliders use the **Avian 2D** physics backend (see [Physics](/docs/r1-alpha7/scripting/physics)); a character with a 2D body and collider will land on, slide along, and be blocked by the marked tiles once the game runs.

### Object collision (trees, houses)

Multi-tile **objects** are not covered by the solid set — they get a proper collision **box**, authored right in the palette:

1. Pick the object in the palette (the multi-cell tree selection).
2. Click the **wall** button. A **green collision box** appears over the selection, initially covering the whole footprint.
3. **Drag its handles** to resize, or **drag inside it** to move — shrink it down to the trunk base. Green means collision, the same as the viewport's collider-edit frame.
4. Pick the **shape** in the dropdown that appears next to the wall button — **Box**, **Circle**, or **Capsule**. The green frame rounds to match. A circle's radius comes from the drawn rect's shorter side; a capsule is vertical, its radius half the rect width with the caps inside the rect's ends.
5. Paint. Every stamped object carries the shape as a real **Collision Shape** component, automatically.

The box is remembered **per palette region** on the `TilemapLayer` (saved with the scene), so re-picking the same tree later recalls its collider, and every future stamp gets it too. Clicking the wall button again with the same pick removes the box. Objects painted *before* the box existed (or before an edit) keep the collider they were stamped with — reshape those individually with the inspector's collider **Edit** toggle in the viewport, or re-stamp them.

Stamped objects also come with **[Y Sort](/docs/r1-alpha7/editor/viewport) already on**, pivoting at their bottom edge — so a character with Y Sort at their feet walks behind the canopy and in front of the trunk with no extra setup. Select an object to see its cyan sort line; the toggle and offset live on the Sprite Image card if you want to change or disable it.

## Selecting painted tiles

Switch the Mode dropdown back to **Select** (or press **Esc**/**Tab**) and the normal 2D picking tools return:

- **Click** a tile or object to select it.
- **Ctrl + click** toggles individual tiles/objects in and out of a **multi-selection** — the way to gather scattered ("stray") tiles so you can move or delete them together. **Shift + click** adds to the selection without toggling.
- **Drag a rubber-band** over empty space to box-select everything it touches.

Selection is pixel-perfect: clicking a fully transparent part of a tile falls through to whatever's behind it.

## Erasing

Besides the momentary **Alt** eraser, the 2D viewport's Mode dropdown has a dedicated **Erase** mode: every left-drag erases (the red cell ghost under the cursor shows which cell), and **Shift + drag** rectangle-erases without needing Alt. Erasing a cell that belongs to a [multi-tile object](#multi-tile-objects) removes the whole object. Erase uses the same active tilemap as Paint, and **Esc** drops back to Select the same way. Erase only appears in the dropdown while the viewport is in 2D view.

While Paint mode is on the 2D selection/drag tools stand down, so painting never accidentally moves a sprite. Painting is edit-mode only — it's disabled in play mode.

Every painted tile is a named child entity of the tilemap — `Tile (x, y)` for a single cell, `Object (x, y)` for a multi-tile composite — saved with the scene like any sprite. The tileset is stored as an asset-relative path, so projects stay portable across machines.
