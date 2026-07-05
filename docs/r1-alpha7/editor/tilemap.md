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
