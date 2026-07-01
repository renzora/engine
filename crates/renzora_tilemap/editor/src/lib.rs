//! Editor-only half of `renzora_tilemap`.
//!
//! `renzora_tilemap` compiles lean (the runtime chunked renderer, no editor
//! deps). This crate adds everything that only matters in the editor:
//!
//! - the **Tilemap** palette panel — shows the selected layer's atlas and picks
//!   the active brush tile by clicking on it (see [`panel`]);
//! - the **Paint** tool + [`paint_tiles`] system — left-drag paints the brush
//!   tile into the selected [`TilemapLayer`], right-drag erases;
//! - **tileset drag-drop** — dropping an image on the panel sets the layer's
//!   tileset;
//! - the **Tilemap** entity preset.
//!
//! Registered via `renzora::add!(TilemapEditorPlugin, Editor)` and linked only by
//! the editor bundle.

mod panel;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{CurrentProject, EditorCamera2d, Node2d, PlayModeState, ViewportBrushActive};
use renzora::{
    AppEditorExt, EditorSelection, EntityPreset, SplashState, ToolEntry, ToolSection,
};
use renzora_tilemap::TilemapLayer;
use renzora_ui::AssetDragPayload;

/// Image extensions accepted as a tileset atlas when dropped on the panel.
const TILESET_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "ktx2", "rmip"];

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

/// Whether the tilemap Paint tool is active. Toggled by the tool button; while
/// on it raises [`ViewportBrushActive`] so the 2D picker stands down.
#[derive(Resource, Default)]
pub struct TilemapPaintMode {
    pub active: bool,
}

#[derive(Default)]
pub struct TilemapEditorPlugin;

impl Plugin for TilemapEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TilemapEditorPlugin");
        app.init_resource::<TilemapBrush>()
            .init_resource::<TilemapPaintMode>()
            .init_resource::<ViewportBrushActive>();

        register_tilemap_preset(app);
        register_paint_tool(app);
        panel::register(app);

        app.add_systems(
            Update,
            (sync_brush_active, paint_tiles, tileset_drop).run_if(in_state(SplashState::Editor)),
        );
    }
}

renzora::add!(TilemapEditorPlugin, Editor);

/// Add the "Tilemap" entry to the Add-Entity picker. Spawns a `Node2d`-tagged
/// entity carrying a default [`TilemapLayer`] — 2D so selecting it auto-switches
/// the viewport to 2D view.
fn register_tilemap_preset(app: &mut App) {
    app.register_entity_preset(EntityPreset {
        id: "tilemap",
        display_name: "Tilemap",
        icon: "grid-four",
        category: "nodes_2d",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Tilemap"),
                    Transform::default(),
                    Visibility::default(),
                    Node2d,
                    TilemapLayer::default(),
                ))
                .id()
        },
    });
}

/// Register the Paint tool. Visible only when a `TilemapLayer` is selected;
/// clicking it toggles paint mode (mirrors the physics "Edit Collider" tool).
fn register_paint_tool(app: &mut App) {
    app.register_tool(
        ToolEntry::new(
            "tilemap.paint",
            "paint-brush",
            "Paint Tiles — left-drag paints, right-drag erases",
            ToolSection::Custom("tilemap"),
        )
        .visible_if(|world| {
            let Some(sel) = world.resource::<EditorSelection>().get() else {
                return false;
            };
            world.get::<TilemapLayer>(sel).is_some()
        })
        .active_if(|world| {
            world
                .get_resource::<TilemapPaintMode>()
                .map(|m| m.active)
                .unwrap_or(false)
        })
        .on_activate(|world| {
            if let Some(mut m) = world.get_resource_mut::<TilemapPaintMode>() {
                m.active = !m.active;
            }
        }),
    );
}

/// Mirror paint mode into the shared [`ViewportBrushActive`] flag so the 2D
/// picker/drag systems stand down while painting.
fn sync_brush_active(paint: Res<TilemapPaintMode>, mut brush_active: ResMut<ViewportBrushActive>) {
    let want = paint.active;
    if brush_active.0 != want {
        brush_active.0 = want;
    }
}

/// Window-cursor → 2D world position through the editor 2D camera + viewport
/// panel rect. `None` if the cursor is outside the panel.
fn cursor_to_world(
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

/// Paint (left) / erase (right) tiles in the selected layer while paint mode is
/// on and we're in 2D edit view. Only mutates the layer when the target cell
/// actually changes, so holding the mouse on one cell doesn't rebuild the mesh
/// every frame.
#[allow(clippy::too_many_arguments)]
fn paint_tiles(
    mouse: Res<ButtonInput<MouseButton>>,
    paint: Res<TilemapPaintMode>,
    brush: Res<TilemapBrush>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play: Option<Res<PlayModeState>>,
    selection: Res<EditorSelection>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
    mut layers: Query<(&mut TilemapLayer, &GlobalTransform)>,
    mut last_cell: Local<Option<IVec2>>,
) {
    if !paint.active
        || play.is_some_and(|p| p.is_in_play_mode())
        || settings.map(|s| s.viewport_view).unwrap_or_default() != ViewportView::Two
    {
        return;
    }
    let painting = mouse.pressed(MouseButton::Left);
    let erasing = mouse.pressed(MouseButton::Right);
    if !painting && !erasing {
        *last_cell = None;
        return;
    }
    let Some(vs) = viewport else { return };
    if !vs.hovered {
        return;
    }
    let Some(entity) = selection.get() else { return };
    let Ok((mut layer, gt)) = layers.get_mut(entity) else {
        return;
    };
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

    // Reading `layer.tile_size` through the `Mut` deref does NOT flag the
    // component as changed; only the `set`/`erase` calls below (which take
    // `&mut self`) do — and we gate those on an actual change.
    let ts = layer.tile_size;
    if ts <= 0.0 {
        return;
    }
    let origin = gt.translation().truncate();
    let local = world - origin;
    let cell = IVec2::new((local.x / ts).floor() as i32, (local.y / ts).floor() as i32);
    if *last_cell == Some(cell) {
        return;
    }
    *last_cell = Some(cell);

    if painting {
        // Stamp the whole brush block. `dx` grows right (+x), `dy` grows down
        // (−y in world), so the atlas's top-left tile lands on the cursor cell
        // and the block reads the same orientation it has in the palette.
        for (dx, dy, idx) in brush.cells() {
            let tc = IVec2::new(cell.x + dx, cell.y - dy);
            if layer.get(tc) != Some(idx) {
                layer.set(tc, idx);
            }
        }
    } else if layer.get(cell).is_some() {
        layer.erase(cell);
    }
}

/// Drop an image asset on the Tilemap panel to set the selected layer's tileset.
fn tileset_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<CurrentProject>>,
    selection: Res<EditorSelection>,
    panel_root: Query<&bevy::ui::RelativeCursorPosition, With<panel::TilemapPanelRoot>>,
    mut layers: Query<&mut TilemapLayer>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached || !payload.matches_extensions(TILESET_EXTENSIONS) {
        return;
    }
    if !panel_root.iter().any(|r| r.cursor_over) {
        return;
    }
    let Some(entity) = selection.get() else { return };
    let Ok(mut layer) = layers.get_mut(entity) else {
        return;
    };
    let path = if let Some(project) = project {
        project.make_asset_relative(&payload.path)
    } else {
        payload.path.to_string_lossy().to_string()
    };
    layer.tileset_path = path;
    // Consume the payload so the viewport's sprite-drop doesn't also fire.
    commands.remove_resource::<AssetDragPayload>();
}
