# Tilemaps

Paint 2D tile-based maps from tileset atlas images. The **Tilemap** panel (2D panel group, icon "grid-four") owns the whole workflow: importing tilesets, switching between the scene's tilemaps, and picking the brush you paint with in the 2D viewport.

## How it works

Tilemaps are two crates working together:

- **`renzora_tilemap`** — the runtime data types. `TilemapLayer` is the authored, scene-saved palette config: the tileset's asset path, the tile size, and how the atlas is sliced. **Every painted tile is a real sprite entity** — a child of the layer entity carrying `TilemapTile` (its grid cell) plus the engine's standard persisted sprite components (`SpriteImagePath`, `SpriteSheet`, `SpriteCustomSize`) — so tiles show in the hierarchy, and they select, move, save, load and animate exactly like hand-placed sprites in both the editor and your shipped game. Registered with `renzora::add!(TilemapPlugin)`.
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
- **Wheel** zooms toward the cursor; **right-drag** pans. The `−` / `+` buttons and the zoom dropdown in the header do the same.
- The **Grid** switch overlays tile boundaries on the atlas.

Selecting tiles **arms the brush** by switching the viewport's **Mode** dropdown to **Paint** — the dropdown (next to the 3D/2D/UI selector) is the single source of truth for the mode. **Tab** toggles Select ↔ Paint while the pointer is over the 2D viewport, and **Esc** drops back to Select.

## Painting

In Paint mode, move the cursor over the 2D viewport — a **semi-transparent ghost of the selected block** follows it, snapped to the active tilemap's grid, showing exactly what a click will stamp and where.

- **Left-drag** paints the block (multi-tile blocks stamp as a unit, keeping the orientation they have in the palette). Strokes are interpolated, so a fast drag never skips cells. Re-painting a cell replaces its tile instead of stacking a second one.
- **Shift + drag** fills a **rectangle**: the press anchors a corner, the drag sizes the region (the ghost previews the tiled fill), and release stamps the brush block tiled across it. Hold **Alt** at the press to rectangle-*erase* instead (the region ghosts red).
- **Alt + left-drag** erases cells. (Erasing is *not* on right-drag — that stays free for the 2D camera pan.)
- **Esc** / **Tab** (or picking **Select** in the Mode dropdown) drops the brush and returns the viewport to normal picking. Deselecting the tilemap in the tab strip does the same.

## Erasing

Besides the momentary **Alt** eraser, the 2D viewport's Mode dropdown has a dedicated **Erase** mode: every left-drag erases (one cell per stroke cell — the red cell ghost under the cursor shows which), and **Shift + drag** rectangle-erases without needing Alt. Erase uses the same active tilemap as Paint, and **Esc** drops back to Select the same way. Erase only appears in the dropdown while the viewport is in 2D view.

While Paint mode is on the 2D selection/drag tools stand down, so painting never accidentally moves a sprite. Painting is edit-mode only — it's disabled in play mode.

Every painted tile is a named child entity of the tilemap (`Tile (x, y)`), saved with the scene like any sprite. The tileset is stored as an asset-relative path, so projects stay portable across machines.
