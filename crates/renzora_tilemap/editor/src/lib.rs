//! Editor-only half of `renzora_tilemap`.
//!
//! `renzora_tilemap` compiles lean (the data types, no editor deps). This
//! crate adds everything that only matters in the editor:
//!
//! - the **Tilemap** panel owns tileset **importing**: drop image(s) on it and
//!   each becomes its own [`TilemapLayer`] entity in the scene (re-dropping a
//!   tileset that's already imported just activates it). There is no
//!   Add-Entity preset — the panel is the one import surface;
//! - **multiple tilemaps**: a tab strip in the panel lists every layer in the
//!   scene and switches [`ActiveTilemap`], which everything else (palette,
//!   brush, painting) keys off — the layer entity does *not* need to stay
//!   selected in the hierarchy while painting. Clicking the active tab again
//!   deselects it (and drops the brush);
//! - selecting tiles in the palette **arms the brush** by switching the
//!   viewport's Mode dropdown to **Paint** (the dropdown is the single source
//!   of truth; **Tab** toggles Scene ↔ Paint over the 2D viewport, **Esc**
//!   drops back to Scene). The selection follows the cursor as a snapped
//!   ghost block (see [`preview`]); left-drag **paints real sprite entities**
//!   (children of the layer, one per cell — see `renzora_tilemap`'s crate
//!   doc) with stroke interpolation, **Shift+drag** fills a rectangle, and
//!   Alt+left-drag erases. Right-drag stays free for the 2D camera pan.
//! - a viewport-toolbar **Randomise** button ([`randomize_selection`]) — the
//!   "make a forest" action. With painted tiles selected in 2D, one click
//!   scatters them to random cells within their own bounding box, turning a
//!   rigid grid into a natural, gappy field. Registered via [`ToolEntry`].
//!
//! Registered via `renzora::add!(TilemapEditorPlugin, Editor)` and linked only by
//! the editor bundle.

mod panel;
mod preview;

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{
    CurrentProject, EditorCamera2d, Node2d, PlayModeState, SpriteAtlasRegion, SpriteCustomSize,
    SpriteImagePath, SpriteSheet, ViewportBrushActive,
};
use renzora::{AppEditorExt, EditorSelection, SplashState, ToolEntry, ToolSection};
use renzora_tilemap::{
    TileObject, TileObjectCell, TileObjectCollider, TilemapLayer, TilemapPaintLayer, TilemapTile,
    TilesetHandle,
};
use renzora_ui::AssetDragPayload;

/// Shared read query over a layer's painted children: each carries its grid
/// cell, and either nothing (a single tile), a [`SpriteAtlasRegion`] (a
/// solid-rectangle object) or a [`TileObject`] (a baked composite object). Used
/// by the paint/erase helpers to find, replace and clear tiles and objects.
type TileQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TilemapTile,
        &'static ChildOf,
        Option<&'static SpriteAtlasRegion>,
        Option<&'static TileObject>,
    ),
>;

/// Image extensions accepted as a tileset atlas when dropped on the panel.
const TILESET_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "ktx2", "rmip"];

/// Whether `path` has a tileset-image extension.
pub(crate) fn is_tileset(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| TILESET_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The tilemap the panel and paint brush operate on. Driven by the panel's tab
/// strip and by hierarchy selection (selecting a `TilemapLayer` entity follows
/// it); painting always writes into this layer, so the user can paint without
/// keeping the entity selected. [`sync_active_tilemap`] keeps it live.
#[derive(Resource, Default)]
pub struct ActiveTilemap(pub Option<Entity>);

/// The **paint layer** strokes write into (a `TilemapPaintLayer` child of the
/// active tilemap, driven by the panel's layer list). `None` = the tilemap
/// root itself — the implicit base layer every pre-layer scene already has,
/// so tilemaps without explicit layers keep painting exactly as before.
#[derive(Resource, Default)]
pub struct ActivePaintLayer(pub Option<Entity>);

/// The current paint brush: the set of atlas cells picked in the palette.
///
/// A plain click/drag selects a solid rectangle; **Ctrl+click** toggles an
/// individual cell and **Shift+click** adds one, so the set can be
/// non-rectangular (a tree's canopy branches over a narrow trunk). `col`/`row`/
/// `w`/`h` are the bounding box of `selected` (kept in sync), and `atlas_cols`
/// is the source atlas's column count so a cell's atlas index can be
/// reconstructed. When `selected` fills the bounding box it's a solid rect and
/// paints via the cheap atlas-crop path; otherwise it bakes a composite object.
#[derive(Resource)]
pub struct TilemapBrush {
    pub col: u32,
    pub row: u32,
    pub w: u32,
    pub h: u32,
    pub atlas_cols: u32,
    /// Picked atlas cells, absolute `(col, row)`. Kept unique.
    pub selected: Vec<UVec2>,
}

impl Default for TilemapBrush {
    fn default() -> Self {
        Self {
            col: 0,
            row: 0,
            w: 1,
            h: 1,
            atlas_cols: 1,
            selected: vec![UVec2::ZERO],
        }
    }
}

impl TilemapBrush {
    /// The picked cells as `(dx, dy, atlas_index)` — `dx`/`dy` are offsets from
    /// the bounding-box top-left (grow right / down), `atlas_index` is the tile.
    pub fn cells(&self) -> Vec<(i32, i32, u32)> {
        let cols = self.atlas_cols.max(1);
        self.selected
            .iter()
            .map(|c| {
                (
                    (c.x - self.col) as i32,
                    (c.y - self.row) as i32,
                    c.y * cols + c.x,
                )
            })
            .collect()
    }

    /// Recompute the bounding box from `selected`. Empty → a zero-size box.
    fn recompute_bounds(&mut self) {
        let Some(first) = self.selected.first() else {
            self.col = 0;
            self.row = 0;
            self.w = 0;
            self.h = 0;
            return;
        };
        let (mut min, mut max) = (*first, *first);
        for c in &self.selected {
            min = min.min(*c);
            max = max.max(*c);
        }
        self.col = min.x;
        self.row = min.y;
        self.w = max.x - min.x + 1;
        self.h = max.y - min.y + 1;
    }

    /// Replace the selection with an explicit set of cells (kept as-is; the
    /// caller guarantees uniqueness). Used by Shift+drag rectangle-add.
    pub fn set_cells(&mut self, cells: Vec<UVec2>, atlas_cols: u32) {
        self.selected = cells;
        self.atlas_cols = atlas_cols;
        self.recompute_bounds();
    }

    /// Replace the selection with the solid rectangle `[c0..=c1] × [r0..=r1]`.
    pub fn set_rect(&mut self, c0: u32, r0: u32, c1: u32, r1: u32, atlas_cols: u32) {
        self.selected.clear();
        for r in r0.min(r1)..=r0.max(r1) {
            for c in c0.min(c1)..=c0.max(c1) {
                self.selected.push(UVec2::new(c, r));
            }
        }
        self.atlas_cols = atlas_cols;
        self.recompute_bounds();
    }

    /// Toggle a single cell in/out of the selection (Ctrl+click). Never empties
    /// the selection — the last cell can't be toggled off.
    pub fn toggle(&mut self, col: u32, row: u32, atlas_cols: u32) {
        let cell = UVec2::new(col, row);
        if let Some(i) = self.selected.iter().position(|&c| c == cell) {
            if self.selected.len() > 1 {
                self.selected.remove(i);
            }
        } else {
            self.selected.push(cell);
        }
        self.atlas_cols = atlas_cols;
        self.recompute_bounds();
    }

    /// Add a single cell to the selection if not already present (Shift+click).
    pub fn add(&mut self, col: u32, row: u32, atlas_cols: u32) {
        let cell = UVec2::new(col, row);
        if !self.selected.contains(&cell) {
            self.selected.push(cell);
        }
        self.atlas_cols = atlas_cols;
        self.recompute_bounds();
    }

    /// Whether the picked cells exactly fill their bounding box (a solid
    /// rectangle) — the cells are unique, so a full count means no holes.
    pub fn is_solid_rect(&self) -> bool {
        self.selected.len() as u32 == self.w.max(1) * self.h.max(1)
    }
}

/// Whether tile painting is live RIGHT NOW. Derived every frame by
/// [`sync_paint_mode`] from the viewport's Mode dropdown (`ViewportMode::Paint`
/// or `ViewportMode::Erase` + an active tilemap) — the dropdown is the single
/// source of truth for the mode; this resource is the cheap bool the
/// paint/preview/brush systems read. While on it raises
/// [`ViewportBrushActive`] so the 2D picker stands down.
#[derive(Resource, Default)]
pub struct TilemapPaintMode {
    pub active: bool,
    /// True in Erase mode: strokes always erase, no Alt needed.
    pub erase: bool,
}

/// An in-flight Shift+drag rectangle fill: `(anchor cell, current cell,
/// erasing)`. Published as a resource (not a `Local`) so the ghost preview can
/// draw the pending region. `None` when no rectangle drag is active.
#[derive(Resource, Default)]
pub struct PaintRectDrag(pub Option<(IVec2, IVec2, bool)>);

/// Hard cap on cells a rectangle fill may touch in one commit. A Shift+drag
/// across a zoomed-out view can span millions of cells; spawning that many
/// entities would hang the editor.
const RECT_FILL_MAX_CELLS: i64 = 16_384;

#[derive(Default)]
pub struct TilemapEditorPlugin;

impl Plugin for TilemapEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TilemapEditorPlugin");
        app.init_resource::<TilemapBrush>()
            .init_resource::<TilemapPaintMode>()
            .init_resource::<ActiveTilemap>()
            .init_resource::<ActivePaintLayer>()
            .init_resource::<ArmedTilesetDrop>()
            .init_resource::<PaintRectDrag>()
            .init_resource::<ViewportBrushActive>()
            .init_resource::<RandomizeState>()
            .init_resource::<TileStrokeSnapshot>();

        panel::register(app);

        // A viewport-toolbar button (2D only, shown once tiles are selected):
        // one click scatters the selection within its own bounds — see
        // [`randomize_selection`]. It's an action, not a mode, so it never reads
        // as "active".
        app.register_tool(
            ToolEntry::new(
                "tilemap.randomise",
                "dice-five",
                "Randomise: scatter the selected tiles within their bounds",
                ToolSection::Custom("tilemap"),
            )
            .visible_if(|w| is_2d_view(w) && selection_has_tiles(w))
            .active_if(|_| false)
            .on_activate(randomize_selection),
        );

        // Chained so painting and the ghost preview see this frame's active
        // tilemap + resolved paint mode (a drop/tab click/mode switch one
        // frame earlier would otherwise lag).
        app.add_systems(
            Update,
            (
                sync_active_tilemap,
                toggle_paint_mode_shortcut,
                escape_to_scene_mode,
                right_click_to_select,
                sync_paint_mode,
                sync_brush_active,
                tilemap_stroke_begin,
                paint_tiles,
                tilemap_stroke_end,
                preview::update_brush_preview,
                arm_tileset_drop,
                commit_tileset_drop,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

renzora::add!(TilemapEditorPlugin, Editor);

// ── Undo integration ─────────────────────────────────────────────────────────
//
// Painted tiles are child sprite entities, so undo can't be a component field
// diff — instead a stroke snapshots the active layer's tile children (their
// *saved* components; `Sprite` is rebuilt by the same observers that hydrate a
// scene load) at press, again at release, and records one `SnapshotCmd`.
// `restore` despawns the layer's current tiles and respawns them from the blob,
// exactly as a scene load does.

/// One painted tile's persistent state (everything that survives scene save;
/// `Sprite` is intentionally omitted — it's rebuilt from these on respawn).
#[derive(Clone, PartialEq)]
struct TileSnap {
    tile: TilemapTile,
    transform: Transform,
    name: String,
    image_path: Option<SpriteImagePath>,
    custom_size: Option<SpriteCustomSize>,
    sheet: Option<SpriteSheet>,
    atlas_region: Option<SpriteAtlasRegion>,
    object: Option<TileObject>,
}

/// A snapshot of one tilemap layer's painted children, keyed by the (stable)
/// layer entity. The `SnapshotCmd` payload for tilemap undo.
#[derive(Clone)]
struct TilemapLayerSnapshot {
    layer: Entity,
    tiles: Vec<TileSnap>,
}

/// Holds the pre-stroke snapshot while a paint gesture is in progress.
#[derive(Resource, Default)]
struct TileStrokeSnapshot {
    before: Option<Vec<TileSnap>>,
    layer: Option<Entity>,
}

/// Collect the saved state of every painted child of `layer`.
fn capture_layer_tiles(world: &mut World, layer: Entity) -> Vec<TileSnap> {
    let mut out = Vec::new();
    let mut q = world.query::<(
        &ChildOf,
        &TilemapTile,
        &Transform,
        Option<&Name>,
        Option<&SpriteImagePath>,
        Option<&SpriteCustomSize>,
        Option<&SpriteSheet>,
        Option<&SpriteAtlasRegion>,
        Option<&TileObject>,
    )>();
    for (parent, tile, tf, name, img, size, sheet, region, obj) in q.iter(world) {
        if parent.parent() != layer {
            continue;
        }
        out.push(TileSnap {
            tile: *tile,
            transform: *tf,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_default(),
            image_path: img.cloned(),
            custom_size: size.cloned(),
            sheet: sheet.cloned(),
            atlas_region: region.cloned(),
            object: obj.cloned(),
        });
    }
    out
}

/// Restore a tilemap layer to a snapshot — the `restore` fn for the tilemap
/// `SnapshotCmd`. Despawns the layer's current tiles and respawns the snapshot's;
/// the sprite-hydration observers rebuild each `Sprite` from the saved
/// components (image path / atlas region / baked object), just like a scene load.
fn restore_tilemap(world: &mut World, snap: &TilemapLayerSnapshot) {
    // Despawn current painted children of the layer.
    let mut to_despawn = Vec::new();
    {
        let mut q = world.query::<(Entity, &ChildOf, &TilemapTile)>();
        for (e, parent, _) in q.iter(world) {
            if parent.parent() == snap.layer {
                to_despawn.push(e);
            }
        }
    }
    for e in to_despawn {
        if let Ok(ent) = world.get_entity_mut(e) {
            ent.despawn();
        }
    }
    // Respawn from the blob. The layer entity itself is untouched (it's not a
    // painted child), so it stays valid across undo.
    for t in &snap.tiles {
        let mut e = world.spawn((
            Name::new(t.name.clone()),
            Node2d,
            t.tile,
            t.transform,
            Visibility::default(),
            ChildOf(snap.layer),
        ));
        if let Some(v) = &t.image_path {
            e.insert(v.clone());
        }
        if let Some(v) = &t.custom_size {
            e.insert(*v);
        }
        if let Some(v) = &t.sheet {
            e.insert(*v);
        }
        if let Some(v) = &t.atlas_region {
            e.insert(*v);
        }
        if let Some(v) = &t.object {
            e.insert(v.clone());
        }
    }
}

/// Snapshot the active layer's tiles when a paint gesture starts (runs before
/// `paint_tiles` so the "before" excludes this frame's first stamp).
fn tilemap_stroke_begin(world: &mut World) {
    if !world
        .resource::<ButtonInput<MouseButton>>()
        .just_pressed(MouseButton::Left)
    {
        return;
    }
    let painting = world
        .get_resource::<TilemapPaintMode>()
        .is_some_and(|p| p.active);
    let hovered = world
        .get_resource::<ViewportState>()
        .is_some_and(|v| v.hovered);
    let Some(layer) = world.get_resource::<ActiveTilemap>().and_then(|a| a.0) else {
        return;
    };
    if !painting || !hovered {
        return;
    }
    let before = capture_layer_tiles(world, layer);
    let mut snap = world.resource_mut::<TileStrokeSnapshot>();
    snap.before = Some(before);
    snap.layer = Some(layer);
}

/// Record the completed stroke on release (runs after `paint_tiles` so the
/// "after" includes any rect-fill committed on the release frame).
fn tilemap_stroke_end(world: &mut World) {
    if !world
        .resource::<ButtonInput<MouseButton>>()
        .just_released(MouseButton::Left)
    {
        return;
    }
    let (before, layer) = {
        let mut snap = world.resource_mut::<TileStrokeSnapshot>();
        (snap.before.take(), snap.layer.take())
    };
    let (Some(before), Some(layer)) = (before, layer) else {
        return;
    };
    let after = capture_layer_tiles(world, layer);
    if before == after {
        return; // click that painted nothing new — no undo step
    }
    let ctx = renzora_undo::active_context(world);
    renzora_undo::record(
        world,
        ctx.clone(),
        Box::new(renzora_undo::SnapshotCmd {
            label: "Paint tiles".to_string(),
            before: TilemapLayerSnapshot { layer, tiles: before },
            after: TilemapLayerSnapshot { layer, tiles: after },
            restore: restore_tilemap,
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &ctx);
}

/// Whether the 2D view is the active viewport view (the Randomise button only
/// makes sense there).
fn is_2d_view(world: &World) -> bool {
    world
        .get_resource::<ViewportSettings>()
        .is_some_and(|s| s.viewport_view == ViewportView::Two)
}

/// Whether the current selection contains at least one painted tile/object, so
/// the Randomise button only appears when it has something to act on.
fn selection_has_tiles(world: &World) -> bool {
    world.get_resource::<EditorSelection>().is_some_and(|s| {
        s.get_all()
            .iter()
            .any(|&e| world.get::<TilemapTile>(e).is_some())
    })
}

/// Advanced once per Randomise click so each press produces a fresh layout.
/// Editor-only and non-deterministic across sessions is fine; a static avoids
/// threading a seed resource through the tool's `&mut World` activator.
static RANDOMIZE_SEED: AtomicU64 = AtomicU64::new(0x243F_6A88_85A3_08D3);

/// SplitMix64 — a tiny, well-distributed integer hash. Derives the per-tile
/// target cell without pulling in a `rand` dependency.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Salt mixed with the per-tile hash + an attempt counter to derive candidate
/// cells (one hash yields both coordinates via `% bw`, `/ bw % bh`).
const RNG_STEP_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// Remembers the last Randomise pass so pressing the dice **again** reshuffles
/// the SAME tiles within the SAME box instead of thinning + shrinking every
/// press (which pulled the whole field inward). Keyed on the selected tile set:
/// a *matching* selection is a repeat (no thinning, cached box); a *changed*
/// selection is a fresh pass (recompute box, may thin a packed block into gaps).
#[derive(Resource, Default)]
struct RandomizeState(Option<RandomizeSnapshot>);

struct RandomizeSnapshot {
    /// Order-independent hash of the tile entities the last pass produced.
    key: u64,
    /// The bounding box (cells) that pass scattered within — reused on repeats
    /// so the field never shrinks.
    min: IVec2,
    max: IVec2,
}

/// A selected painted tile gathered by [`randomize_selection`]: its entity, its
/// object footprint (`None` for a plain single tile), and its layer's tile size.
struct RandomTile {
    entity: Entity,
    footprint: Option<(u32, u32)>,
    tile_size: f32,
}

/// The on-grid local position (layer space) a painted tile/object sits at,
/// mirroring the stamp formulas in [`paint_tiles`]: a single tile centres on its
/// cell; a composite object (footprint `w × h`) anchors at its top-left cell and
/// extends right (+x) and up (−y from the anchor).
fn grid_base_local(cell: IVec2, footprint: Option<(u32, u32)>, ts: f32) -> Vec2 {
    if let Some((w, h)) = footprint {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        Vec2::new(
            cell.x as f32 * ts + w * ts * 0.5,
            (cell.y as f32 - h + 1.0) * ts + h * ts * 0.5,
        )
    } else {
        Vec2::new(cell.x as f32 * ts + ts * 0.5, cell.y as f32 * ts + ts * 0.5)
    }
}

/// Order-independent hash of a set of tile entities — the [`RandomizeState`] key
/// identifying "the same selection" across presses.
fn tile_set_key(entities: &[Entity]) -> u64 {
    // XOR-fold per-entity hashes so order doesn't matter and no sort is needed.
    entities
        .iter()
        .fold(0u64, |acc, e| acc ^ splitmix64(e.to_bits()))
}

/// **Randomise** the current viewport selection: move every selected tile/object
/// to a random cell **within the selection's own bounding box** — the "grid of
/// trees → forest" action, driven by the viewport-toolbar dice button.
///
/// The catch pressing it repeatedly must avoid is collapsing the field inward.
/// So the first press on a given selection **thins** it (tiles that collide on a
/// random cell are dropped, turning a packed block into a ~63%-coverage scatter
/// with natural gaps) and records the selection's bounding box in
/// [`RandomizeState`]. Every later press on that same (thinned) selection is a
/// **repeat**: it reuses the cached box and places the tiles at *unique* cells —
/// no more deletion and no shrinking box — so it just reshuffles the same trees
/// into a fresh layout. Changing the selection starts a new pass.
///
/// `TilemapTile`, `Transform` and the entity `Name` all move to the new cell so
/// the data stays truthful (erase/repaint by cell keep working). Only real
/// painted tiles are touched; anything else in a mixed selection is left as-is.
/// Runs as a deferred `&mut World` tool activator. Wraps the scatter in an undo
/// snapshot of the active layer so the whole reshuffle is one Ctrl+Z step.
fn randomize_selection(world: &mut World) {
    let layer = world.get_resource::<ActiveTilemap>().and_then(|a| a.0);
    let before = layer.map(|l| capture_layer_tiles(world, l));
    randomize_selection_inner(world);
    if let (Some(layer), Some(before)) = (layer, before) {
        let after = capture_layer_tiles(world, layer);
        if before != after {
            let ctx = renzora_undo::active_context(world);
            renzora_undo::record(
                world,
                ctx.clone(),
                Box::new(renzora_undo::SnapshotCmd {
                    label: "Randomise tiles".to_string(),
                    before: TilemapLayerSnapshot { layer, tiles: before },
                    after: TilemapLayerSnapshot { layer, tiles: after },
                    restore: restore_tilemap,
                    merge_key: None,
                }),
            );
            renzora_undo::seal(world, &ctx);
        }
    }
}

fn randomize_selection_inner(world: &mut World) {
    let Some(selected) = world.get_resource::<EditorSelection>().map(|s| s.get_all()) else {
        return;
    };
    if selected.is_empty() {
        return;
    }

    // Pass 1: collect the painted tiles (footprint + tile size) and the bounding
    // box of their cells. Non-tile entities are kept selected, untouched.
    let mut tiles: Vec<RandomTile> = Vec::new();
    let mut others: Vec<Entity> = Vec::new();
    let mut min = IVec2::splat(i32::MAX);
    let mut max = IVec2::splat(i32::MIN);
    for &e in &selected {
        let cell = match world.get::<TilemapTile>(e) {
            Some(t) => IVec2::new(t.x, t.y),
            None => {
                others.push(e);
                continue;
            }
        };
        let layer_e = match world.get::<ChildOf>(e) {
            Some(c) => c.parent(),
            None => {
                others.push(e);
                continue;
            }
        };
        let ts = match world.get::<TilemapLayer>(layer_e) {
            Some(l) => l.tile_size,
            None => {
                others.push(e);
                continue;
            }
        };
        if ts <= 0.0 {
            others.push(e);
            continue;
        }
        let footprint = world
            .get::<SpriteAtlasRegion>(e)
            .map(|r| (r.w, r.h))
            .or_else(|| world.get::<TileObject>(e).map(|o| (o.w, o.h)));
        min = min.min(cell);
        max = max.max(cell);
        tiles.push(RandomTile {
            entity: e,
            footprint,
            tile_size: ts,
        });
    }
    if tiles.is_empty() {
        return;
    }

    // Is this the same selection as the last pass? If so, reuse its box and don't
    // thin (repeat); otherwise it's a fresh pass over this selection's own box.
    let tile_ids: Vec<Entity> = tiles.iter().map(|t| t.entity).collect();
    let key = tile_set_key(&tile_ids);
    let cached = world
        .get_resource::<RandomizeState>()
        .and_then(|s| s.0.as_ref())
        .filter(|snap| snap.key == key)
        .map(|snap| (snap.min, snap.max));
    let (bmin, bmax, thin) = match cached {
        Some((mn, mx)) => (mn, mx, false),
        None => (min, max, true),
    };

    let seed = splitmix64(RANDOMIZE_SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed));
    let bw = (bmax.x - bmin.x + 1).max(1) as u64;
    let bh = (bmax.y - bmin.y + 1).max(1) as u64;
    // Candidate cell for a tile's `base` hash on attempt `a` (one hash → both
    // axes). Attempt varies the hash so a rejected cell retries elsewhere.
    let cell_for = |base: u64, a: u64| {
        let h = splitmix64(base ^ RNG_STEP_SALT.wrapping_mul(a + 1));
        IVec2::new(
            bmin.x + (h % bw) as i32,
            bmin.y + ((h / bw) % bh) as i32,
        )
    };

    // Pass 2: place each tile. On a fresh pass a collision drops the tile (gaps);
    // on a repeat we probe for a free cell (no deletion — count stays put).
    let mut occupied: HashSet<(i32, i32)> = HashSet::with_capacity(tiles.len());
    let mut survivors = others;
    let mut placed_tiles: Vec<Entity> = Vec::with_capacity(tiles.len());
    for t in &tiles {
        let base = splitmix64(t.entity.to_bits() ^ seed);
        let cell = if thin {
            let c = cell_for(base, 0);
            if !occupied.insert((c.x, c.y)) {
                world.despawn(t.entity);
                continue;
            }
            c
        } else {
            // Repeat: find a free cell (up to 32 tries), else fall back to the
            // first candidate and allow the rare overlap rather than deleting.
            let mut chosen = cell_for(base, 0);
            for a in 0..32 {
                let c = cell_for(base, a);
                if occupied.insert((c.x, c.y)) {
                    chosen = c;
                    break;
                }
            }
            occupied.insert((chosen.x, chosen.y));
            chosen
        };
        let g = grid_base_local(cell, t.footprint, t.tile_size);
        if let Some(mut tf) = world.get_mut::<Transform>(t.entity) {
            tf.translation.x = g.x;
            tf.translation.y = g.y;
        }
        if let Some(mut tile) = world.get_mut::<TilemapTile>(t.entity) {
            tile.x = cell.x;
            tile.y = cell.y;
        }
        if let Some(mut name) = world.get_mut::<Name>(t.entity) {
            *name = Name::new(if t.footprint.is_some() {
                format!("Object ({}, {})", cell.x, cell.y)
            } else {
                format!("Tile ({}, {})", cell.x, cell.y)
            });
        }
        placed_tiles.push(t.entity);
        survivors.push(t.entity);
    }

    // Remember this pass keyed on the tiles that survived, so the next press on
    // the same set is recognised as a repeat (reuse box, don't thin again).
    if let Some(mut state) = world.get_resource_mut::<RandomizeState>() {
        state.0 = Some(RandomizeSnapshot {
            key: tile_set_key(&placed_tiles),
            min: bmin,
            max: bmax,
        });
    }
    if survivors.len() != selected.len() {
        if let Some(selection) = world.get_resource::<EditorSelection>() {
            selection.set_multiple(survivors);
        }
    }
}

/// Keep [`ActiveTilemap`] pointing at a live layer: follow hierarchy selection
/// when it lands on a tilemap and drop despawned entities. Deliberately does
/// NOT auto-adopt a layer when none is active — "no active tilemap" is a real
/// state (the user deselected via the tab strip); while in it painting stays
/// dormant (see [`sync_paint_mode`]) so the viewport behaves normally.
fn sync_active_tilemap(
    selection: Res<EditorSelection>,
    layers: Query<Entity, With<TilemapLayer>>,
    mut active: ResMut<ActiveTilemap>,
) {
    if selection.is_changed() {
        if let Some(e) = selection.get() {
            if layers.contains(e) && active.0 != Some(e) {
                active.0 = Some(e);
            }
        }
    }
    if let Some(e) = active.0 {
        if !layers.contains(e) {
            active.0 = None;
        }
    }
}

/// Derive [`TilemapPaintMode`] from the viewport's Mode dropdown: painting is
/// live while the mode is **Paint** or **Erase** and a tilemap is active.
/// The dropdown is the single source of truth — the palette arms it by
/// setting the mode (see `select_tiles_from_atlas`), Esc/Tab switch it back,
/// and there is no separate toolbar button to disagree with it.
fn sync_paint_mode(
    settings: Option<Res<ViewportSettings>>,
    active: Res<ActiveTilemap>,
    mut paint: ResMut<TilemapPaintMode>,
) {
    use renzora::core::viewport_types::ViewportMode;
    let mode = settings.map(|s| s.viewport_mode).unwrap_or_default();
    let want = matches!(mode, ViewportMode::Paint | ViewportMode::Erase) && active.0.is_some();
    let erase = mode == ViewportMode::Erase;
    if paint.active != want {
        paint.active = want;
    }
    if paint.erase != erase {
        paint.erase = erase;
    }
}

/// Mirror paint mode into the shared [`ViewportBrushActive`] flag so the 2D
/// picker/drag systems stand down while painting.
fn sync_brush_active(paint: Res<TilemapPaintMode>, mut brush_active: ResMut<ViewportBrushActive>) {
    let want = paint.active;
    if brush_active.0 != want {
        brush_active.0 = want;
    }
}

/// **Tab** toggles Scene ↔ Paint mode while the pointer is over the 2D
/// viewport and a tilemap is active — the keyboard mirror of the header's
/// Mode dropdown. Gated on viewport hover so Tab keeps its meaning in text
/// fields and other panels.
fn toggle_paint_mode_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    active: Res<ActiveTilemap>,
    mut settings: Option<ResMut<ViewportSettings>>,
) {
    use renzora::core::viewport_types::ViewportMode;
    if !keys.just_pressed(KeyCode::Tab) || active.0.is_none() {
        return;
    }
    if !viewport.is_some_and(|v| v.hovered) {
        return;
    }
    let Some(settings) = settings.as_deref_mut() else {
        return;
    };
    if settings.viewport_view != ViewportView::Two {
        return;
    }
    settings.viewport_mode = if settings.viewport_mode == ViewportMode::Paint {
        ViewportMode::Scene
    } else {
        ViewportMode::Paint
    };
}

/// Esc drops the brush/eraser by switching the viewport mode back to Select.
fn escape_to_scene_mode(
    keys: Res<ButtonInput<KeyCode>>,
    paint: Res<TilemapPaintMode>,
    mut settings: Option<ResMut<ViewportSettings>>,
) {
    use renzora::core::viewport_types::ViewportMode;
    if !paint.active || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Some(settings) = settings.as_deref_mut() {
        if matches!(
            settings.viewport_mode,
            ViewportMode::Paint | ViewportMode::Erase
        ) {
            settings.viewport_mode = ViewportMode::Scene;
        }
    }
}

/// A right-*click* (press + release with no meaningful movement) over the 2D
/// viewport drops the brush/eraser back to Select — a quick "get out of paint
/// mode" without reaching for Esc/Tab or the dropdown. A right-*drag* is the 2D
/// camera pan and must be preserved, so the switch only fires when the cursor
/// barely moved between press and release; any drift past [`RIGHT_CLICK_DRAG_PX`]
/// marks it a pan and cancels the switch.
fn right_click_to_select(
    mouse: Res<ButtonInput<MouseButton>>,
    viewport: Option<Res<ViewportState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut settings: Option<ResMut<ViewportSettings>>,
    // Press position (window px) + whether the hold has become a drag.
    mut press: Local<Option<Vec2>>,
    mut dragged: Local<bool>,
) {
    use renzora::core::viewport_types::ViewportMode;
    /// Movement (window px) past which a right hold is a pan, not a click.
    const RIGHT_CLICK_DRAG_PX: f32 = 5.0;

    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    if mouse.just_pressed(MouseButton::Right) {
        *press = cursor;
        *dragged = false;
    }
    // Latch "this became a pan" as soon as the cursor leaves the click radius,
    // so a drag that returns near the start still counts as a pan.
    if mouse.pressed(MouseButton::Right) && !*dragged {
        if let (Some(p), Some(c)) = (*press, cursor) {
            if p.distance(c) > RIGHT_CLICK_DRAG_PX {
                *dragged = true;
            }
        }
    }
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    let was_click = !*dragged && press.is_some();
    *press = None;
    *dragged = false;
    if !was_click {
        return; // it was a pan — leave the mode alone
    }
    // Only when the pointer is over the 2D viewport and we're actually painting.
    if !viewport.is_some_and(|v| v.hovered) {
        return;
    }
    let Some(settings) = settings.as_deref_mut() else {
        return;
    };
    if settings.viewport_view != ViewportView::Two {
        return;
    }
    if matches!(
        settings.viewport_mode,
        ViewportMode::Paint | ViewportMode::Erase
    ) {
        settings.viewport_mode = ViewportMode::Scene;
    }
}

/// Window-cursor → 2D world position through the editor 2D camera + viewport
/// panel rect. `None` if the cursor is outside the panel.
pub(crate) fn cursor_to_world(
    cursor: Vec2,
    vs: &ViewportState,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<Vec2> {
    let in_rect = cursor - vs.screen_position;
    if in_rect.x < 0.0
        || in_rect.y < 0.0
        || in_rect.x >= vs.screen_size.x
        || in_rect.y >= vs.screen_size.y
    {
        return None;
    }
    let image_size = vs.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return None;
    }
    let scaled = Vec2::new(
        in_rect.x * image_size.x / vs.screen_size.x,
        in_rect.y * image_size.y / vs.screen_size.y,
    );
    camera.viewport_to_world_2d(cam_gt, scaled).ok()
}

/// Paint (left-drag) or erase (Alt+left-drag) tiles in the active layer while
/// paint mode is on and we're in 2D edit view. Right-drag is deliberately NOT
/// an eraser — it belongs to the 2D camera pan.
///
/// Every painted tile is a real sprite entity, child of the layer: `Sprite`
/// bound to the tileset (via the persisted `SpriteImagePath`), `SpriteSheet`
/// picking the atlas frame (the engine derives `Sprite.rect` from it), and
/// [`TilemapTile`] recording the grid cell so re-painting a cell replaces its
/// tile instead of stacking a second one.
///
/// Strokes are **interpolated**: each frame stamps every cell on the line
/// from the previous cell to the current one, so a fast drag can't skip
/// cells and leave holes. **Shift+drag** switches to a rectangle fill — the
/// press anchors a corner, the drag sizes the region (the ghost preview shows
/// it), and release fills it by tiling the brush block (or erases it when Alt
/// was held at press).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn paint_tiles(
    // Tupled: a bare system tops out at 16 params and this one needs 17.
    (mouse, keys): (Res<ButtonInput<MouseButton>>, Res<ButtonInput<KeyCode>>),
    paint: Res<TilemapPaintMode>,
    brush: Res<TilemapBrush>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play: Option<Res<PlayModeState>>,
    // Tupled (the system rides the 16-param cap): the active tilemap, which
    // paint layer strokes write into, and the query to validate/lock-check it.
    (active, active_layer, paint_layers): (
        Res<ActiveTilemap>,
        Res<ActivePaintLayer>,
        Query<(&TilemapPaintLayer, &ChildOf)>,
    ),
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
    layers: Query<(&TilemapLayer, &TilesetHandle, &GlobalTransform)>,
    tiles: TileQuery,
    mut sheets: Query<&mut SpriteSheet>,
    mut rect_drag: ResMut<PaintRectDrag>,
    mut commands: Commands,
    // Tupled Locals: `last_cell` gates stroke interpolation for 1×1 tiles;
    // `last_object` tracks the last multi-tile object's anchor so a drag tiles
    // objects edge-to-edge instead of stamping one per cell; `painting` latches
    // whether the current mouse-hold is an actual paint gesture (press began over
    // the viewport) vs. a drag that merely wandered in. (The system is at the
    // 16-param cap, so the Locals share one slot.)
    (mut last_cell, mut last_object, mut painting): (
        Local<Option<IVec2>>,
        Local<Option<IVec2>>,
        Local<bool>,
    ),
) {
    if !paint.active
        || play.is_some_and(|p| p.is_in_play_mode())
        || settings.map(|s| s.viewport_view).unwrap_or_default() != ViewportView::Two
    {
        if rect_drag.0.is_some() {
            rect_drag.0 = None;
        }
        // Clear stroke state so re-entering Paint starts fresh — a stale
        // `last_object` anchor must not gate the first click of a new stroke.
        *last_cell = None;
        *last_object = None;
        return;
    }
    let Some(layer_entity) = active.0 else {
        rect_drag.0 = None;
        return;
    };
    let Ok((layer, tileset, gt)) = layers.get(layer_entity) else {
        rect_drag.0 = None;
        return;
    };
    let ts = layer.tile_size;
    if ts <= 0.0 {
        return;
    }
    // Strokes write into the active PAINT LAYER when it belongs to this
    // tilemap; otherwise the tilemap root (the implicit base layer). A locked
    // layer swallows the stroke entirely — no fallback to another layer, that
    // would paint somewhere the user isn't looking.
    let paint_root = match active_layer.0.and_then(|e| paint_layers.get(e).ok().map(|p| (e, p))) {
        Some((e, (pl, child_of))) if child_of.parent() == layer_entity => {
            if pl.locked {
                return;
            }
            e
        }
        _ => layer_entity,
    };
    // The atlas grid — needed to size the tile's `SpriteSheet`. Wait for the
    // image so a half-loaded atlas doesn't bake a wrong hframes/vframes.
    let Some(img_size) = images.get(&tileset.image).map(|i| i.size_f32()) else {
        return;
    };
    let atlas_px = layer.atlas_tile_px.max(1) as f32;
    let tile_px = layer.atlas_tile_px.max(1);
    let cols = layer.effective_columns(img_size.x).max(1);
    let rows = ((img_size.y / atlas_px).floor() as u32).max(1);
    // More than one picked cell paints a single composite "object" sprite (a
    // tree/house) rather than one sprite per cell. A solid-rectangle pick uses
    // the cheap atlas-crop object (`stamp_object`); a non-rectangular pick bakes
    // a texture (`stamp_tile_object`). One cell → ordinary per-cell tiling.
    let object_brush = brush.selected.len() > 1;
    // The palette-authored collision box for this pick (if the user set one
    // via the panel's wall button) — stamped objects carry it as an entity
    // collider so they block 2D bodies out of the box.
    let obj_collider = layer.collider_for(brush.col, brush.row, brush.w.max(1), brush.h.max(1));

    // Shared per-cell ops (`fn`s, not closures — both need `commands`/`sheets`
    // mutably and are called from several paths below).
    #[allow(clippy::too_many_arguments)]
    fn stamp_cell(
        tc: IVec2,
        idx: u32,
        cols: u32,
        rows: u32,
        ts: f32,
        layer_entity: Entity,
        image: &Handle<Image>,
        path: &str,
        tiles: &TileQuery,
        sheets: &mut Query<&mut SpriteSheet>,
        commands: &mut Commands,
    ) {
        // Only a *single* tile (no composite object) at this cell is the replace
        // target — an object here is left for the object paths.
        let existing = tiles
            .iter()
            .find(|(_, t, p, region, object)| {
                region.is_none()
                    && object.is_none()
                    && p.parent() == layer_entity
                    && t.x == tc.x
                    && t.y == tc.y
            })
            .map(|(e, _, _, _, _)| e);
        if let Some(existing) = existing {
            // Re-painting a cell just swaps the frame — cheaper than a
            // despawn/respawn and keeps any user tweaks on the entity.
            if let Ok(mut sheet) = sheets.get_mut(existing) {
                if sheet.hframes != cols || sheet.vframes != rows || sheet.frame != idx {
                    *sheet = SpriteSheet {
                        hframes: cols,
                        vframes: rows,
                        frame: idx,
                    };
                }
                return;
            }
            // No SpriteSheet (shouldn't happen for painted tiles) — rebuild.
            commands.entity(existing).try_despawn();
        }
        commands.spawn((
            Name::new(format!("Tile ({}, {})", tc.x, tc.y)),
            Node2d,
            TilemapTile { x: tc.x, y: tc.y },
            // Sprites are centre-anchored; the cell's min corner is at
            // cell * tile_size in the layer's local space.
            Transform::from_xyz(tc.x as f32 * ts + ts * 0.5, tc.y as f32 * ts + ts * 0.5, 0.0),
            Visibility::default(),
            Sprite {
                image: image.clone(),
                custom_size: Some(Vec2::splat(ts)),
                ..default()
            },
            SpriteImagePath(path.to_string()),
            SpriteCustomSize(Vec2::splat(ts)),
            SpriteSheet {
                hframes: cols,
                vframes: rows,
                frame: idx,
            },
            ChildOf(layer_entity),
        ));
    }
    fn erase_cell(tc: IVec2, layer_entity: Entity, tiles: &TileQuery, commands: &mut Commands) {
        // Erase the single tile at this exact cell, plus any composite object
        // whose footprint covers it — so erasing anywhere on a stamped tree
        // deletes the whole tree, not a phantom cell it doesn't own.
        for (e, t, p, region, object) in tiles.iter() {
            if p.parent() != layer_entity {
                continue;
            }
            let hit = match entity_footprint(region, object) {
                Some((w, h)) => object_covers(IVec2::new(t.x, t.y), w, h, tc),
                None => t.x == tc.x && t.y == tc.y,
            };
            if hit {
                commands.entity(e).try_despawn();
            }
        }
    }
    /// Clear what a new object at top-left cell `c` (footprint `w × h`) will sit
    /// on: an object already anchored on `c` (clean re-stamp) and any loose
    /// single tiles inside the footprint (so the object doesn't render tangled
    /// with same-z tiles). Overlapping *other* objects are left alone — a drag
    /// lays them down on purpose.
    fn clear_under(
        c: IVec2,
        w: u32,
        h: u32,
        layer_entity: Entity,
        tiles: &TileQuery,
        commands: &mut Commands,
    ) {
        for (e, t, p, region, object) in tiles.iter() {
            if p.parent() != layer_entity {
                continue;
            }
            let remove = if region.is_some() || object.is_some() {
                t.x == c.x && t.y == c.y
            } else {
                object_covers(c, w, h, IVec2::new(t.x, t.y))
            };
            if remove {
                commands.entity(e).try_despawn();
            }
        }
    }
    /// Spawn one composite object for a **solid rectangular** pick, anchored at
    /// top-left cell `c`. A single sprite cropped to the atlas block (persisted
    /// via [`SpriteAtlasRegion`]) — the cheap path that shares the atlas
    /// texture, used when the pick has no holes.
    #[allow(clippy::too_many_arguments)]
    fn stamp_object(
        c: IVec2,
        brush: &TilemapBrush,
        tile_px: u32,
        ts: f32,
        layer_entity: Entity,
        image: &Handle<Image>,
        path: &str,
        collider: Option<TileObjectCollider>,
        tiles: &TileQuery,
        commands: &mut Commands,
    ) {
        let w = brush.w.max(1);
        let h = brush.h.max(1);
        clear_under(c, w, h, layer_entity, tiles, commands);
        let cw = w as f32 * ts;
        let ch = h as f32 * ts;
        // `c` is the block's TOP-LEFT cell (palette orientation): it extends
        // right (+x) and down (−y in world), matching the per-tile paint below.
        let center_x = c.x as f32 * ts + cw * 0.5;
        let center_y = (c.y as f32 - h as f32 + 1.0) * ts + ch * 0.5;
        let px = tile_px.max(1) as f32;
        // Same edge inset the engine's crop uses, so a fractional zoom can't
        // bleed the neighbouring atlas cell across the block's outer edge.
        const EDGE_INSET: f32 = 0.05;
        let rect = Rect::new(
            brush.col as f32 * px + EDGE_INSET,
            brush.row as f32 * px + EDGE_INSET,
            (brush.col + w) as f32 * px - EDGE_INSET,
            (brush.row + h) as f32 * px - EDGE_INSET,
        );
        let spawned = commands.spawn((
            Name::new(format!("Object ({}, {})", c.x, c.y)),
            Node2d,
            TilemapTile { x: c.x, y: c.y },
            Transform::from_xyz(center_x, center_y, 0.0),
            Visibility::default(),
            // Objects y-sort out of the box, pivoting at their BOTTOM edge —
            // a tree sorts by its base, so a character above it draws behind
            // the canopy and one below draws in front. Toggleable per entity
            // in the inspector's Sprite Image card.
            renzora::core::YSort {
                offset: -ch * 0.5,
                ..Default::default()
            },
            Sprite {
                image: image.clone(),
                custom_size: Some(Vec2::new(cw, ch)),
                rect: Some(rect),
                ..default()
            },
            SpriteImagePath(path.to_string()),
            SpriteCustomSize(Vec2::new(cw, ch)),
            SpriteAtlasRegion {
                col: brush.col,
                row: brush.row,
                w,
                h,
                tile_px: tile_px.max(1),
            },
            ChildOf(layer_entity),
        ))
        .id();
        // Palette-authored collision box → real entity collider. The sprite
        // routes the shape to the 2D physics backend; the non-default values
        // opt it out of auto-fit (which fits factory-default shapes only).
        if let Some(col) = collider {
            commands.entity(spawned).insert(col.shape_data(ts));
        }
    }
    /// Spawn one composite object for a **non-rectangular** pick (scattered
    /// cells, e.g. a canopy over a narrow trunk), anchored at top-left cell `c`.
    /// Records the picked cells in a [`TileObject`]; the runtime baker builds
    /// the transparent-gap texture and inserts the `Sprite`, so this is one
    /// entity that shows only the tiles the user chose.
    #[allow(clippy::too_many_arguments)]
    fn stamp_tile_object(
        c: IVec2,
        brush: &TilemapBrush,
        tile_px: u32,
        ts: f32,
        layer_entity: Entity,
        path: &str,
        collider: Option<TileObjectCollider>,
        tiles: &TileQuery,
        commands: &mut Commands,
    ) {
        let w = brush.w.max(1);
        let h = brush.h.max(1);
        clear_under(c, w, h, layer_entity, tiles, commands);
        let cells: Vec<TileObjectCell> = brush
            .cells()
            .into_iter()
            .map(|(dx, dy, _idx)| TileObjectCell {
                dx: dx as u32,
                dy: dy as u32,
                col: brush.col + dx as u32,
                row: brush.row + dy as u32,
            })
            .collect();
        let cw = w as f32 * ts;
        let ch = h as f32 * ts;
        let center_x = c.x as f32 * ts + cw * 0.5;
        let center_y = (c.y as f32 - h as f32 + 1.0) * ts + ch * 0.5;
        let spawned = commands
            .spawn((
                Name::new(format!("Object ({}, {})", c.x, c.y)),
                Node2d,
                TilemapTile { x: c.x, y: c.y },
                Transform::from_xyz(center_x, center_y, 0.0),
                Visibility::default(),
                // Bottom-edge y-sort, same as the atlas-crop object path.
                renzora::core::YSort {
                    offset: -ch * 0.5,
                    ..Default::default()
                },
                SpriteCustomSize(Vec2::new(cw, ch)),
                TileObject {
                    tileset_path: path.to_string(),
                    tile_px: tile_px.max(1),
                    w,
                    h,
                    cells,
                },
                ChildOf(layer_entity),
            ))
            .id();
        // Same palette-authored collider as the atlas-crop path above.
        if let Some(col) = collider {
            commands.entity(spawned).insert(col.shape_data(ts));
        }
    }
    /// Stamp one object at top-left cell `c`, picking the cheap atlas-crop path
    /// for a solid-rectangle pick and the baked-texture path otherwise.
    #[allow(clippy::too_many_arguments)]
    fn stamp_auto(
        c: IVec2,
        brush: &TilemapBrush,
        tile_px: u32,
        ts: f32,
        layer_entity: Entity,
        image: &Handle<Image>,
        path: &str,
        collider: Option<TileObjectCollider>,
        tiles: &TileQuery,
        commands: &mut Commands,
    ) {
        if brush.is_solid_rect() {
            stamp_object(
                c, brush, tile_px, ts, layer_entity, image, path, collider, tiles, commands,
            );
        } else {
            stamp_tile_object(
                c, brush, tile_px, ts, layer_entity, path, collider, tiles, commands,
            );
        }
    }

    // Release edge: commit a pending rectangle fill. Runs before the hover
    // guards on purpose — releasing with the cursor off the panel must still
    // commit (the region was authored in-world while dragging).
    if !mouse.pressed(MouseButton::Left) {
        *last_cell = None;
        *last_object = None;
        *painting = false;
        if let Some((a, b, erase)) = rect_drag.0.take() {
            let min = a.min(b);
            let max = a.max(b);
            let count = (max.x - min.x + 1) as i64 * (max.y - min.y + 1) as i64;
            if count > RECT_FILL_MAX_CELLS {
                warn!(
                    "[tilemap] rectangle fill skipped: {count} cells exceeds the {RECT_FILL_MAX_CELLS} cap"
                );
                return;
            }
            let bw = brush.w.max(1) as i32;
            let bh = brush.h.max(1) as i32;
            let bcols = brush.atlas_cols.max(1);
            if erase {
                for y in min.y..=max.y {
                    for x in min.x..=max.x {
                        erase_cell(IVec2::new(x, y), paint_root, &tiles, &mut commands);
                    }
                }
            } else if object_brush {
                // Multi-tile object brush: tile whole objects on a block-sized
                // lattice from the region's TOP-LEFT (min.x, max.y), so the
                // fill reads as a field of trees/houses, not sliced cells.
                let mut y = max.y;
                while y >= min.y {
                    let mut x = min.x;
                    while x <= max.x {
                        stamp_auto(
                            IVec2::new(x, y), &brush, tile_px, ts, paint_root,
                            &tileset.image, &layer.tileset_path, obj_collider, &tiles,
                            &mut commands,
                        );
                        x += bw;
                    }
                    y -= bh;
                }
            } else {
                for y in min.y..=max.y {
                    for x in min.x..=max.x {
                        // Tile the brush pattern from the region's TOP-LEFT
                        // (min.x, max.y) so it reads in palette orientation.
                        let dx = (x - min.x).rem_euclid(bw) as u32;
                        let dy = (max.y - y).rem_euclid(bh) as u32;
                        let idx = (brush.row + dy) * bcols + (brush.col + dx);
                        stamp_cell(
                            IVec2::new(x, y), idx, cols, rows, ts, paint_root, &tileset.image,
                            &layer.tileset_path, &tiles, &mut sheets, &mut commands,
                        );
                    }
                }
            }
        }
        return;
    }

    // Erase mode makes every stroke an erase; in Paint mode Alt is the
    // momentary eraser, as before.
    let erasing = paint.erase || keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let Some(vs) = viewport else { return };
    // Only a press that BEGAN over the viewport arms a stroke. `hovered` merely
    // tracks the cursor, so without this a mouse-hold for something else — dragging
    // a dock panel/tab, a scrollbar — that passes over the 2D viewport would start
    // painting. Latch the gesture at press time from where the press landed.
    if mouse.just_pressed(MouseButton::Left) {
        *painting = vs.hovered;
    }
    if !*painting {
        return;
    }
    if !vs.hovered && rect_drag.0.is_none() {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };
    let Some(world) = cursor_to_world(cursor, &vs, camera, cam_gt) else {
        return;
    };
    let origin = gt.translation().truncate();
    let local = world - origin;
    let cell = IVec2::new((local.x / ts).floor() as i32, (local.y / ts).floor() as i32);

    // Shift at press anchors a rectangle; the drag only sizes it (commit is
    // on release, above). Erase intent is captured at press so toggling Alt
    // mid-drag doesn't flip the region's meaning.
    if rect_drag.0.is_some() || (mouse.just_pressed(MouseButton::Left) && shift) {
        let (anchor, erase) = rect_drag.0.map(|(a, _, e)| (a, e)).unwrap_or((cell, erasing));
        rect_drag.0 = Some((anchor, cell, erase));
        *last_cell = Some(cell);
        return;
    }

    if *last_cell == Some(cell) {
        return;
    }
    // Interpolate the stroke: stamp every cell between the previous frame's
    // cell and this one, so a fast drag can't out-run the frame rate and
    // leave holes.
    let from = last_cell.unwrap_or(cell);
    *last_cell = Some(cell);
    if erasing {
        for c in line_cells(from, cell) {
            erase_cell(c, paint_root, &tiles, &mut commands);
        }
    } else if object_brush {
        // Multi-tile object brush: stamp ONE object per position. Only stamp
        // once the cursor has left the last object's footprint, so a drag
        // tiles objects edge-to-edge instead of smearing one per cell; a fresh
        // click always stamps (`last_object` is cleared on release). No
        // interpolation — objects are discrete placements, not a continuous
        // stroke.
        if last_object.is_none_or(|a| !object_covers(a, brush.w, brush.h, cell)) {
            stamp_auto(
                cell, &brush, tile_px, ts, paint_root, &tileset.image,
                &layer.tileset_path, obj_collider, &tiles, &mut commands,
            );
            *last_object = Some(cell);
        }
    } else {
        for c in line_cells(from, cell) {
            // Stamp the whole brush block. `dx` grows right (+x), `dy` grows
            // down (−y in world), so the atlas's top-left tile lands on the
            // cursor cell and the block reads the same orientation it has in
            // the palette.
            for (dx, dy, idx) in brush.cells() {
                let tc = IVec2::new(c.x + dx, c.y - dy);
                stamp_cell(
                    tc, idx, cols, rows, ts, paint_root, &tileset.image,
                    &layer.tileset_path, &tiles, &mut sheets, &mut commands,
                );
            }
        }
    }
}

/// A painted child's object footprint in cells, if it's a composite object —
/// from either a solid-rectangle [`SpriteAtlasRegion`] or a baked
/// [`TileObject`]. `None` means it's an ordinary single tile.
fn entity_footprint(region: Option<&SpriteAtlasRegion>, object: Option<&TileObject>) -> Option<(u32, u32)> {
    region
        .map(|r| (r.w, r.h))
        .or_else(|| object.map(|o| (o.w, o.h)))
}

/// Whether the multi-tile object anchored at top-left cell `anchor`, with a
/// `w × h` footprint that extends right (+x) and down (−y in world, palette
/// orientation), covers `cell`. Used for erase-the-whole-object, clean
/// re-stamp, and the drag stamp gate.
fn object_covers(anchor: IVec2, w: u32, h: u32, cell: IVec2) -> bool {
    let (w, h) = (w.max(1) as i32, h.max(1) as i32);
    cell.x >= anchor.x && cell.x < anchor.x + w && cell.y <= anchor.y && cell.y > anchor.y - h
}

/// All grid cells on the line segment `a → b`, inclusive (Bresenham). Used to
/// interpolate paint strokes between frames.
fn line_cells(a: IVec2, b: IVec2) -> Vec<IVec2> {
    let mut out = Vec::new();
    let d = (b - a).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = d.x - d.y;
    let mut c = a;
    loop {
        out.push(c);
        if c == b {
            break;
        }
        let e2 = 2 * err;
        if e2 > -d.y {
            err -= d.y;
            c.x += sx;
        }
        if e2 < d.x {
            err += d.x;
            c.y += sy;
        }
    }
    out
}

/// Tileset paths captured while a compatible drag hovers the panel. The asset
/// browser removes [`AssetDragPayload`] via a deferred command on mouse-up, and
/// an intervening exclusive system can flush that removal before a
/// release-frame read would see it — so (mirroring the viewport's armed drop)
/// the candidate is snapshotted every hover frame and consumed on release.
#[derive(Resource, Default)]
struct ArmedTilesetDrop(Option<Vec<std::path::PathBuf>>);

/// Every frame: arm the drop with the payload's image paths while a detached
/// drag hovers the Tilemap panel; disarm when it hovers elsewhere. When no
/// payload exists (the release frame) the snapshot is left for the commit.
fn arm_tileset_drop(
    payload: Option<Res<AssetDragPayload>>,
    panel_root: Query<&bevy::ui::RelativeCursorPosition, With<panel::TilemapPanelRoot>>,
    mut armed: ResMut<ArmedTilesetDrop>,
) {
    let Some(payload) = payload else {
        return; // keep the last snapshot for the release frame
    };
    if !payload.is_detached || !panel_root.iter().any(|r| r.cursor_over) {
        armed.0 = None;
        return;
    }
    // A multi-select drag imports every image in it (non-images are skipped).
    let images: Vec<std::path::PathBuf> = payload
        .paths
        .iter()
        .filter(|p| is_tileset(p))
        .cloned()
        .collect();
    armed.0 = (!images.is_empty()).then_some(images);
}

/// On the release edge, import the armed tileset(s): every dropped image
/// becomes its own [`TilemapLayer`] entity named after the file, and the last
/// one imported becomes the active tilemap. Re-dropping a tileset a layer
/// already uses doesn't duplicate it — it just activates that layer.
fn commit_tileset_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    project: Option<Res<CurrentProject>>,
    layers: Query<(Entity, &TilemapLayer)>,
    mut armed: ResMut<ArmedTilesetDrop>,
    mut active: ResMut<ActiveTilemap>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(paths) = armed.0.take() else { return };

    for abs in &paths {
        let path = if let Some(project) = project.as_ref() {
            project.make_asset_relative(abs)
        } else {
            abs.to_string_lossy().to_string()
        };
        if let Some((existing, _)) = layers.iter().find(|(_, l)| l.tileset_path == path) {
            active.0 = Some(existing);
            continue;
        }
        let name = abs
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Tilemap")
            .to_string();
        let id = commands
            .spawn((
                Name::new(name),
                Transform::default(),
                Visibility::default(),
                Node2d,
                TilemapLayer {
                    tileset_path: path,
                    ..default()
                },
            ))
            .id();
        active.0 = Some(id);
    }
}
