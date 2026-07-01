//! The **Tilemap** palette panel — a zoomable/pannable tile selector.
//!
//! The tileset atlas is shown inside an ember [`scroll_view_xy`] (both
//! scrollbars). On top of it:
//! - **wheel** zooms toward the cursor; **right-drag** pans; the zoom is also
//!   driven by the `−` / `+` buttons and the zoom dropdown in the header;
//! - **left-drag** selects a rectangle of tiles (a click selects one); the
//!   selection is shown with a highlight box + a corner handle you can drag to
//!   grow it; the block is stamped as a unit by the paint tool;
//! - a **Grid** switch overlays tile boundaries.
//!
//! The atlas image is swapped reactively via a single-item `keyed_list` keyed on
//! the tileset path + dimensions; the grid overlay is managed separately (see
//! [`sync_grid_overlay`]) so toggling it doesn't rebuild the image (which would
//! reset the scroll position).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, ScrollPosition};

use renzora::{EditorSelection, RenzoraShellExt, SplashState};
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, keyed_list, KeyedSnapshot};
use renzora_ember::theme::text_muted;
use renzora_ember::widgets::{dropdown, icon_button, scroll_view_xy, toggle_switch, EmberScroll, ScrollThumb};
use renzora_tilemap::{TilemapLayer, TilesetHandle};

use crate::TilemapBrush;

/// Marker on the panel's content root — the full-panel tileset drop target.
#[derive(Component)]
pub(crate) struct TilemapPanelRoot;

/// Marker on the atlas `ImageNode` — the click-to-pick + zoom target.
#[derive(Component)]
struct TilesetImageNode;

/// Marker on the selected-tile highlight box (a child of the atlas image).
#[derive(Component)]
struct TileHighlight;

/// Marker on the highlight's corner resize handle.
#[derive(Component)]
struct SelectionHandle;

/// Marker on a grid-overlay line (child of the atlas image).
#[derive(Component)]
struct GridLine;

/// Zoom step buttons.
#[derive(Component)]
struct ZoomInBtn;
#[derive(Component)]
struct ZoomOutBtn;

/// Accent colour for the selection highlight + handle.
const ACCENT: Color = Color::srgb(1.0, 0.6, 0.1);

/// Discrete zoom levels offered by the dropdown / snapped to by its readout.
const ZOOM_PRESETS: [f32; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
const ZOOM_LABELS: [&str; 7] = ["25%", "50%", "100%", "200%", "400%", "800%", "1600%"];

/// Atlas zoom (pixels-per-source-pixel). Persisted across panel rebuilds.
#[derive(Resource)]
struct TilemapZoom(f32);

impl Default for TilemapZoom {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Whether the atlas grid overlay is shown.
#[derive(Resource, Default)]
struct GridOn(bool);

/// Index of the preset nearest to `z` (for the dropdown readout).
fn nearest_zoom_index(z: f32) -> usize {
    ZOOM_PRESETS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (**a - z)
                .abs()
                .partial_cmp(&(**b - z).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(2)
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<TilemapZoom>();
    app.init_resource::<GridOn>();
    app.register_shell_panel("tilemap", "Tilemap", "grid-four", "2D");
    app.register_panel_content("tilemap", false, build);
    app.add_systems(
        Update,
        (
            zoom_pan_atlas,
            zoom_step_buttons,
            apply_atlas_zoom,
            update_tile_highlight,
            sync_grid_overlay,
            select_tiles_from_atlas,
            resize_selection,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Root fills the whole panel so a tileset can be dropped anywhere on it.
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            TilemapPanelRoot,
            RelativeCursorPosition::default(),
        ))
        .id();

    // Header: Grid switch on the left, zoom controls on the right.
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let grid_icon = icon_text(commands, &fonts.phosphor, "grid-four", text_muted(), 14.0);
    let grid_switch = toggle_switch(commands, false);
    bind_2way(
        commands,
        grid_switch,
        |world: &World| world.get_resource::<GridOn>().map(|g| g.0).unwrap_or(false),
        |world: &mut World, v: &bool| {
            if let Some(mut g) = world.get_resource_mut::<GridOn>() {
                g.0 = *v;
            }
        },
    );

    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    let zoom_out = icon_button(commands, fonts, "minus");
    commands.entity(zoom_out).insert(ZoomOutBtn);
    let labels: Vec<&str> = ZOOM_LABELS.to_vec();
    let zoom_dd = dropdown(commands, fonts, &labels, nearest_zoom_index(1.0));
    bind_2way(
        commands,
        zoom_dd,
        |world: &World| {
            nearest_zoom_index(world.get_resource::<TilemapZoom>().map(|z| z.0).unwrap_or(1.0))
        },
        |world: &mut World, idx: &usize| {
            let z = ZOOM_PRESETS.get(*idx).copied().unwrap_or(1.0);
            if let Some(mut tz) = world.get_resource_mut::<TilemapZoom>() {
                tz.0 = z;
            }
        },
    );
    let zoom_in = icon_button(commands, fonts, "plus");
    commands.entity(zoom_in).insert(ZoomInBtn);

    commands
        .entity(header)
        .add_children(&[grid_icon, grid_switch, spacer, zoom_out, zoom_dd, zoom_in]);

    // Atlas content — `align_self: Start` + `flex_shrink: 0` so it keeps its
    // native (zoomed) size and can overflow both axes of the scroll viewport.
    let content = commands
        .spawn(Node {
            align_self: AlignSelf::Start,
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    keyed_list(commands, content, atlas_snapshot);

    // Ember both-axis scroll view, filling the panel's remaining height.
    let scroll = scroll_view_xy(commands, content);
    let scroll_box = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();
    commands.entity(scroll_box).add_child(scroll);

    commands.entity(root).add_children(&[header, scroll_box]);
    root
}

/// Single-item snapshot: the selected layer's atlas image + its selection
/// highlight (with resize handle), keyed on tileset path + dimensions. The grid
/// overlay is intentionally NOT part of this — see [`sync_grid_overlay`].
fn atlas_snapshot(world: &World) -> KeyedSnapshot {
    let mut items = Vec::new();
    let mut data: Option<(Handle<Image>, Vec2)> = None;

    let sel = world
        .get_resource::<EditorSelection>()
        .and_then(|s| s.get());
    if let Some(e) = sel {
        if let Some(tileset) = world.get::<TilesetHandle>(e) {
            let px = world
                .get_resource::<Assets<Image>>()
                .and_then(|imgs| imgs.get(&tileset.image))
                .map(|img| {
                    let s = img.size_f32();
                    Vec2::new(s.x.max(1.0), s.y.max(1.0))
                })
                .unwrap_or(Vec2::ONE);
            if world.get::<TilemapLayer>(e).is_some() {
                let mut hasher = DefaultHasher::new();
                world.get::<TilemapLayer>(e).unwrap().tileset_path.hash(&mut hasher);
                (px.x as u64).hash(&mut hasher);
                (px.y as u64).hash(&mut hasher);
                items.push((1u64, hasher.finish()));
                data = Some((tileset.image.clone(), px));
            }
        }
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, _fonts: &EmberFonts, _i: usize| {
            let (handle, px) = data.clone().expect("build only runs for emitted items");
            let img = c
                .spawn((
                    ImageNode::new(handle),
                    Node {
                        width: Val::Px(px.x),
                        height: Val::Px(px.y),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    TilesetImageNode,
                    RelativeCursorPosition::default(),
                ))
                .id();

            // Selection highlight (positioned each frame in percent, so it scales
            // with zoom automatically) + a corner handle to drag-expand it.
            let hl = c
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor::all(ACCENT),
                    TileHighlight,
                    bevy::picking::Pickable::IGNORE,
                    ChildOf(img),
                ))
                .id();
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(-5.0),
                    bottom: Val::Px(-5.0),
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(ACCENT),
                Interaction::default(),
                SelectionHandle,
                ChildOf(hl),
            ));

            img
        }),
    }
}

/// Spawn/despawn the grid overlay to match [`GridOn`], as children of the atlas
/// image — without rebuilding the image (so the scroll position is preserved).
/// Also re-spawns after the image is rebuilt for a new tileset (which despawns
/// the old lines with it).
fn sync_grid_overlay(
    grid: Res<GridOn>,
    selection: Res<EditorSelection>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    image_node: Query<Entity, With<TilesetImageNode>>,
    lines: Query<Entity, With<GridLine>>,
    mut commands: Commands,
) {
    let want = grid.0;
    let have = !lines.is_empty();
    if want == have {
        return;
    }
    if !want {
        for l in &lines {
            commands.entity(l).try_despawn();
        }
        return;
    }
    // want && !have → spawn lines under the current image.
    let Ok(img) = image_node.single() else { return };
    let Some(e) = selection.get() else { return };
    let Ok((layer, tileset)) = layers.get(e) else { return };
    let Some(image) = images.get(&tileset.image) else { return };
    let s = image.size_f32();
    let cols = layer.effective_columns(s.x).max(1);
    let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
    let color = Color::srgba(1.0, 1.0, 1.0, 0.18);
    for i in 1..cols {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(i as f32 / cols as f32 * 100.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(color),
            bevy::picking::Pickable::IGNORE,
            GridLine,
            ChildOf(img),
        ));
    }
    for j in 1..rows {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(j as f32 / rows as f32 * 100.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(color),
            bevy::picking::Pickable::IGNORE,
            GridLine,
            ChildOf(img),
        ));
    }
}

/// Wheel zooms toward the cursor; right-drag pans. Both drive the ember scroll
/// viewport's offset directly (found by walking image → content → viewport).
fn zoom_pan_atlas(
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    mut zoom: ResMut<TilemapZoom>,
    atlas: Query<Entity, With<TilesetImageNode>>,
    parents: Query<&ChildOf>,
    mut viewports: Query<(
        &mut EmberScroll,
        &mut ScrollPosition,
        &ComputedNode,
        &RelativeCursorPosition,
    )>,
) {
    let Ok(img) = atlas.single() else {
        wheel.clear();
        motion.clear();
        return;
    };
    let Some(viewport) = parents
        .get(img)
        .ok()
        .map(|c| c.parent())
        .and_then(|content| parents.get(content).ok().map(|c| c.parent()))
    else {
        wheel.clear();
        motion.clear();
        return;
    };
    let Ok((mut es, mut sp, cn, rcp)) = viewports.get_mut(viewport) else {
        wheel.clear();
        motion.clear();
        return;
    };
    let over = rcp.cursor_over;
    let inv = cn.inverse_scale_factor();
    let vsize = cn.size() * inv;
    let cursor = rcp.normalized.map(|n| (n + Vec2::splat(0.5)) * vsize);

    let mut wy = 0.0;
    for e in wheel.read() {
        wy += e.y;
    }
    if over && wy != 0.0 {
        if let Some(cur) = cursor {
            let old = zoom.0;
            let new = (old * 1.15f32.powf(wy)).clamp(0.1, 16.0);
            if (new - old).abs() > 1e-4 {
                zoom.0 = new;
                let r = new / old;
                let nx = ((sp.x + cur.x) * r - cur.x).max(0.0);
                let ny = ((sp.y + cur.y) * r - cur.y).max(0.0);
                sp.x = nx;
                sp.y = ny;
                es.set_offset_xy(nx, ny);
            }
        }
    }

    if over && mouse.pressed(MouseButton::Right) {
        let mut d = Vec2::ZERO;
        for e in motion.read() {
            d += e.delta;
        }
        if d != Vec2::ZERO {
            let nx = (sp.x - d.x).max(0.0);
            let ny = (sp.y - d.y).max(0.0);
            sp.x = nx;
            sp.y = ny;
            es.set_offset_xy(nx, ny);
        }
    } else {
        motion.clear();
    }
}

/// The `−` / `+` zoom buttons step the zoom by a fixed factor.
fn zoom_step_buttons(
    zoom_in: Query<&Interaction, (With<ZoomInBtn>, Changed<Interaction>)>,
    zoom_out: Query<&Interaction, (With<ZoomOutBtn>, Changed<Interaction>)>,
    mut zoom: ResMut<TilemapZoom>,
) {
    if zoom_in.iter().any(|i| *i == Interaction::Pressed) {
        zoom.0 = (zoom.0 * 1.25).clamp(0.1, 16.0);
    }
    if zoom_out.iter().any(|i| *i == Interaction::Pressed) {
        zoom.0 = (zoom.0 / 1.25).clamp(0.1, 16.0);
    }
}

/// Drive the atlas image's on-screen size from the zoom (native × zoom).
fn apply_atlas_zoom(
    zoom: Res<TilemapZoom>,
    selection: Res<EditorSelection>,
    images: Res<Assets<Image>>,
    tilesets: Query<&TilesetHandle>,
    mut image_nodes: Query<&mut Node, With<TilesetImageNode>>,
) {
    let Ok(mut node) = image_nodes.single_mut() else {
        return;
    };
    let Some(e) = selection.get() else { return };
    let Ok(tileset) = tilesets.get(e) else { return };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let s = img.size_f32();
    let w = Val::Px(s.x * zoom.0);
    let h = Val::Px(s.y * zoom.0);
    if node.width != w {
        node.width = w;
    }
    if node.height != h {
        node.height = h;
    }
}

/// Position the selection-highlight box over the current brush rect (in percent
/// of the atlas, so it scales with zoom).
fn update_tile_highlight(
    brush: Res<TilemapBrush>,
    selection: Res<EditorSelection>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    mut highlight: Query<&mut Node, With<TileHighlight>>,
) {
    let Ok(mut node) = highlight.single_mut() else {
        return;
    };
    let Some(e) = selection.get() else { return };
    let Ok((layer, tileset)) = layers.get(e) else {
        return;
    };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let s = img.size_f32();
    let cols = layer.effective_columns(s.x).max(1);
    let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
    let col = brush.col.min(cols - 1);
    let row = brush.row.min(rows - 1);
    let bw = brush.w.clamp(1, cols - col);
    let bh = brush.h.clamp(1, rows - row);
    let left = Val::Percent(col as f32 / cols as f32 * 100.0);
    let top = Val::Percent(row as f32 / rows as f32 * 100.0);
    let w = Val::Percent(bw as f32 / cols as f32 * 100.0);
    let h = Val::Percent(bh as f32 / rows as f32 * 100.0);
    if node.left != left {
        node.left = left;
    }
    if node.top != top {
        node.top = top;
    }
    if node.width != w {
        node.width = w;
    }
    if node.height != h {
        node.height = h;
    }
}

/// Left-drag a rectangle across the atlas to select a block of tiles (a single
/// click selects one). Skips scrollbar + resize-handle interactions so grabbing
/// those doesn't start a selection.
fn select_tiles_from_atlas(
    mouse: Res<ButtonInput<MouseButton>>,
    selection: Res<EditorSelection>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas: Query<&RelativeCursorPosition, With<TilesetImageNode>>,
    thumbs: Query<&Interaction, With<ScrollThumb>>,
    handles: Query<&Interaction, With<SelectionHandle>>,
    mut brush: ResMut<TilemapBrush>,
    mut start: Local<Option<(u32, u32)>>,
    mut blocked: Local<bool>,
) {
    if !mouse.pressed(MouseButton::Left) {
        *start = None;
        *blocked = false;
        return;
    }
    // On press, decide whether this drag belongs to a scrollbar or the resize
    // handle — if so, don't select for the rest of the hold.
    if mouse.just_pressed(MouseButton::Left) {
        let on_bar = thumbs
            .iter()
            .any(|i| matches!(i, Interaction::Pressed | Interaction::Hovered));
        let on_handle = handles.iter().any(|i| *i == Interaction::Pressed);
        *blocked = on_bar || on_handle;
    }
    if *blocked {
        return;
    }
    let Some(rcp) = atlas.iter().find(|r| r.cursor_over) else {
        return;
    };
    let Some(n) = rcp.normalized else { return };
    let Some(entity) = selection.get() else { return };
    let Ok((layer, tileset)) = layers.get(entity) else {
        return;
    };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let size = img.size_f32();
    let cols = layer.effective_columns(size.x).max(1);
    let atlas_px = layer.atlas_tile_px.max(1) as f32;
    let rows = ((size.y / atlas_px).floor() as u32).max(1);

    let u = (n.x + 0.5).clamp(0.0, 0.999);
    let v = (n.y + 0.5).clamp(0.0, 0.999);
    let cur = (
        (u * cols as f32).floor() as u32,
        (v * rows as f32).floor() as u32,
    );
    if mouse.just_pressed(MouseButton::Left) {
        *start = Some(cur);
    }
    let s = start.unwrap_or(cur);
    let (c0, c1) = (s.0.min(cur.0), s.0.max(cur.0));
    let (r0, r1) = (s.1.min(cur.1), s.1.max(cur.1));
    brush.col = c0;
    brush.row = r0;
    brush.w = c1 - c0 + 1;
    brush.h = r1 - r0 + 1;
    brush.atlas_cols = cols;
}

/// Drag the corner handle to grow the selection from its fixed top-left corner.
fn resize_selection(
    handles: Query<&Interaction, With<SelectionHandle>>,
    selection: Res<EditorSelection>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas: Query<&RelativeCursorPosition, With<TilesetImageNode>>,
    mut brush: ResMut<TilemapBrush>,
) {
    if !handles.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Ok(rcp) = atlas.single() else { return };
    let Some(n) = rcp.normalized else { return };
    let Some(e) = selection.get() else { return };
    let Ok((layer, tileset)) = layers.get(e) else {
        return;
    };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let s = img.size_f32();
    let cols = layer.effective_columns(s.x).max(1);
    let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
    let u = (n.x + 0.5).clamp(0.0, 0.999);
    let v = (n.y + 0.5).clamp(0.0, 0.999);
    let cur_col = (u * cols as f32).floor() as u32;
    let cur_row = (v * rows as f32).floor() as u32;
    brush.w = cur_col.saturating_sub(brush.col) + 1;
    brush.h = cur_row.saturating_sub(brush.row) + 1;
    brush.atlas_cols = cols;
}
