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
//!
//! Registered via `renzora::add!(TilemapEditorPlugin, Editor)` and linked only by
//! the editor bundle.

mod panel;
mod preview;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{
    CurrentProject, EditorCamera2d, Node2d, PlayModeState, SpriteCustomSize, SpriteImagePath,
    SpriteSheet, ViewportBrushActive,
};
use renzora::{EditorSelection, SplashState};
use renzora_tilemap::{TilemapLayer, TilemapTile, TilesetHandle};
use renzora_ui::AssetDragPayload;

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

/// The current paint brush: a rectangular block of atlas tiles selected in the
/// palette (drag to select more than one; a single click is a 1×1 block).
/// `atlas_cols` is the column count of the atlas the selection came from, so the
/// per-cell atlas index can be reconstructed when stamping.
#[derive(Resource)]
pub struct TilemapBrush {
    pub col: u32,
    pub row: u32,
    pub w: u32,
    pub h: u32,
    pub atlas_cols: u32,
}

impl Default for TilemapBrush {
    fn default() -> Self {
        Self {
            col: 0,
            row: 0,
            w: 1,
            h: 1,
            atlas_cols: 1,
        }
    }
}

impl TilemapBrush {
    /// The block's cells as `(dx, dy, atlas_index)` — `dx`/`dy` are offsets from
    /// the stamp origin (grow right / down), `atlas_index` is the tile to place.
    pub fn cells(&self) -> Vec<(i32, i32, u32)> {
        let cols = self.atlas_cols.max(1);
        let mut out = Vec::with_capacity((self.w * self.h) as usize);
        for dy in 0..self.h.max(1) {
            for dx in 0..self.w.max(1) {
                let idx = (self.row + dy) * cols + (self.col + dx);
                out.push((dx as i32, dy as i32, idx));
            }
        }
        out
    }
}

/// Whether tile painting is live RIGHT NOW. Derived every frame by
/// [`sync_paint_mode`] from the viewport's Mode dropdown (`ViewportMode::Paint`
/// + an active tilemap) — the dropdown is the single source of truth for the
/// mode; this resource is the cheap bool the paint/preview/brush systems read.
/// While on it raises [`ViewportBrushActive`] so the 2D picker stands down.
#[derive(Resource, Default)]
pub struct TilemapPaintMode {
    pub active: bool,
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
            .init_resource::<ArmedTilesetDrop>()
            .init_resource::<PaintRectDrag>()
            .init_resource::<ViewportBrushActive>();

        panel::register(app);

        // Chained so painting and the ghost preview see this frame's active
        // tilemap + resolved paint mode (a drop/tab click/mode switch one
        // frame earlier would otherwise lag).
        app.add_systems(
            Update,
            (
                sync_active_tilemap,
                toggle_paint_mode_shortcut,
                escape_to_scene_mode,
                sync_paint_mode,
                sync_brush_active,
                paint_tiles,
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
/// live while the mode is **Paint** and a tilemap is active. The dropdown is
/// the single source of truth — the palette arms it by setting the mode (see
/// `select_tiles_from_atlas`), Esc/Tab switch it back, and there is no
/// separate toolbar button to disagree with it.
fn sync_paint_mode(
    settings: Option<Res<ViewportSettings>>,
    active: Res<ActiveTilemap>,
    mut paint: ResMut<TilemapPaintMode>,
) {
    let want = settings
        .map(|s| s.viewport_mode == renzora::core::viewport_types::ViewportMode::Paint)
        .unwrap_or(false)
        && active.0.is_some();
    if paint.active != want {
        paint.active = want;
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

/// Esc drops the brush by switching the viewport mode back to Scene.
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
        if settings.viewport_mode == ViewportMode::Paint {
            settings.viewport_mode = ViewportMode::Scene;
        }
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
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
    layers: Query<(&TilemapLayer, &TilesetHandle, &GlobalTransform)>,
    tiles: Query<(Entity, &TilemapTile, &ChildOf)>,
    mut sheets: Query<&mut SpriteSheet>,
    mut rect_drag: ResMut<PaintRectDrag>,
    mut commands: Commands,
    mut last_cell: Local<Option<IVec2>>,
) {
    if !paint.active
        || play.is_some_and(|p| p.is_in_play_mode())
        || settings.map(|s| s.viewport_view).unwrap_or_default() != ViewportView::Two
    {
        if rect_drag.0.is_some() {
            rect_drag.0 = None;
        }
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
    // The atlas grid — needed to size the tile's `SpriteSheet`. Wait for the
    // image so a half-loaded atlas doesn't bake a wrong hframes/vframes.
    let Some(img_size) = images.get(&tileset.image).map(|i| i.size_f32()) else {
        return;
    };
    let atlas_px = layer.atlas_tile_px.max(1) as f32;
    let cols = layer.effective_columns(img_size.x).max(1);
    let rows = ((img_size.y / atlas_px).floor() as u32).max(1);

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
        tiles: &Query<(Entity, &TilemapTile, &ChildOf)>,
        sheets: &mut Query<&mut SpriteSheet>,
        commands: &mut Commands,
    ) {
        let existing = tiles
            .iter()
            .find(|(_, t, p)| p.parent() == layer_entity && t.x == tc.x && t.y == tc.y)
            .map(|(e, _, _)| e);
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
    fn erase_cell(
        tc: IVec2,
        layer_entity: Entity,
        tiles: &Query<(Entity, &TilemapTile, &ChildOf)>,
        commands: &mut Commands,
    ) {
        if let Some((e, _, _)) = tiles
            .iter()
            .find(|(_, t, p)| p.parent() == layer_entity && t.x == tc.x && t.y == tc.y)
        {
            commands.entity(e).try_despawn();
        }
    }

    // Release edge: commit a pending rectangle fill. Runs before the hover
    // guards on purpose — releasing with the cursor off the panel must still
    // commit (the region was authored in-world while dragging).
    if !mouse.pressed(MouseButton::Left) {
        *last_cell = None;
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
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let tc = IVec2::new(x, y);
                    if erase {
                        erase_cell(tc, layer_entity, &tiles, &mut commands);
                    } else {
                        // Tile the brush pattern from the region's TOP-LEFT
                        // (min.x, max.y) so it reads in palette orientation.
                        let dx = (x - min.x).rem_euclid(bw) as u32;
                        let dy = (max.y - y).rem_euclid(bh) as u32;
                        let idx = (brush.row + dy) * bcols + (brush.col + dx);
                        stamp_cell(
                            tc, idx, cols, rows, ts, layer_entity, &tileset.image,
                            &layer.tileset_path, &tiles, &mut sheets, &mut commands,
                        );
                    }
                }
            }
        }
        return;
    }

    let erasing = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let Some(vs) = viewport else { return };
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
    for c in line_cells(from, cell) {
        if erasing {
            erase_cell(c, layer_entity, &tiles, &mut commands);
        } else {
            // Stamp the whole brush block. `dx` grows right (+x), `dy` grows
            // down (−y in world), so the atlas's top-left tile lands on the
            // cursor cell and the block reads the same orientation it has in
            // the palette.
            for (dx, dy, idx) in brush.cells() {
                let tc = IVec2::new(c.x + dx, c.y - dy);
                stamp_cell(
                    tc, idx, cols, rows, ts, layer_entity, &tileset.image,
                    &layer.tileset_path, &tiles, &mut sheets, &mut commands,
                );
            }
        }
    }
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
