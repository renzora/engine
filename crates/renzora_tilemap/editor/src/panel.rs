//! The **Tilemap** panel — tileset importing, a tilemap tab strip, and a
//! zoomable/pannable tile palette.
//!
//! The panel is the import surface for tilemaps: dropping tileset image(s) on
//! it spawns one [`TilemapLayer`] per image (see `tileset_drop` in the crate
//! root), and a **tab strip** lists every tilemap in the scene, switching
//! [`ActiveTilemap`]. The active tilemap's atlas is shown inside an ember
//! [`scroll_view_xy`] (both scrollbars). On top of it:
//! - **wheel** zooms toward the cursor; **right-drag** pans; the zoom is also
//!   driven by the `−` / `+` buttons and the zoom dropdown in the header;
//! - **left-drag** selects a rectangle of tiles (a click selects one); the
//!   selection is shown with a highlight box + a corner handle you can drag to
//!   grow it, and **arms the paint brush** — the block follows the cursor in
//!   the 2D viewport and stamps as a unit;
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

use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
use renzora::{RenzoraShellExt, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{accent, rgb, tab_active, text_muted, text_primary};
use renzora_ember::widgets::{
    dropdown, icon_button, label, scroll_view_xy, toggle_switch, EmberScroll, HoverTooltip,
    ScrollThumb, ScrollbarBusy,
};
use renzora_tilemap::{
    TileObjectCollider, TilemapLayer, TilemapPaintLayer, TilesetHandle, PAINT_LAYER_Z_STEP,
};

use crate::{ActivePaintLayer, ActiveTilemap, TilemapBrush};

/// Marker on the panel's content root — the full-panel tileset drop target.
#[derive(Component)]
pub(crate) struct TilemapPanelRoot;

/// Marker on a tab in the tilemap tab strip; holds the layer entity it selects.
#[derive(Component)]
struct TilemapTab(Entity);

/// Marker on the full-panel drop indicator overlay, shown while a tileset drag
/// hovers the panel.
#[derive(Component)]
struct TilesetDropIndicator;

/// Per-frame mirror of the scene's tilemaps (entity + display name), sorted for
/// a stable tab order. Reactive snapshot closures only get `&World` and can't
/// run a fresh query, so [`mirror_tilemap_list`] maintains this instead.
#[derive(Resource, Default)]
struct TilemapList(Vec<(Entity, String)>);

/// One row of the paint-layer list, mirrored per frame for the reactive list
/// (same `&World`-snapshot limitation as [`TilemapList`]). `entity` is `None`
/// for the implicit **Base** row — the tilemap root itself.
#[derive(Clone, PartialEq)]
struct PaintLayerRowData {
    entity: Option<Entity>,
    name: String,
    order: i32,
    visible: bool,
    locked: bool,
}

/// The active tilemap's paint layers (+ the Base row), topmost first —
/// mirroring Godot's list where the top row draws on top.
#[derive(Resource, Default)]
struct PaintLayerList(Vec<PaintLayerRowData>);

/// Marker on a paint-layer row; carries which layer it selects (`None` = Base).
#[derive(Component)]
struct PaintLayerRow(Option<Entity>);

/// Visibility (eye) toggle inside a paint-layer row.
#[derive(Component)]
struct PaintLayerEye(Entity);

/// Lock toggle inside a paint-layer row.
#[derive(Component)]
struct PaintLayerLock(Entity);

/// Reorder buttons inside a paint-layer row (`up` = draw later/on top).
#[derive(Component)]
struct PaintLayerReorder {
    layer: Entity,
    up: bool,
}

/// The "+ add layer" button at the end of the layer list.
#[derive(Component)]
struct AddPaintLayerBtn;

/// Marker on the atlas `ImageNode` — the click-to-pick + zoom target.
#[derive(Component)]
struct TilesetImageNode;

/// Marker on the selected-tile highlight box (a child of the atlas image).
#[derive(Component)]
struct TileHighlight;

/// Marker on the highlight's corner resize handle.
#[derive(Component)]
struct SelectionHandle;

/// Marker on a per-cell selection tint (child of the atlas image). Shown for a
/// non-rectangular pick so the user sees exactly which cells are selected — the
/// bounding-box highlight alone can't convey holes.
#[derive(Component)]
struct TileCellMark;

/// Marker on a grid-overlay line (child of the atlas image).
#[derive(Component)]
struct GridLine;

/// Header button: toggle **collision** on the current palette pick. A single
/// cell toggles membership in `TilemapLayer.solid_tiles` (merged tile
/// colliders); a multi-cell object pick toggles an editable collision box
/// (`TilemapLayer.object_colliders`) stamped onto painted objects.
#[derive(Component)]
struct MarkSolidBtn;

/// Marker on a per-cell solid-collision tint (child of the atlas image) —
/// shows which palette cells are marked solid, independent of the selection.
#[derive(Component)]
struct SolidCellMark;

/// Marker on every node of the palette collision-box editor (the green rect +
/// its handles), for wholesale despawn and for blocking tile selection while
/// the cursor interacts with it.
#[derive(Component)]
struct PaletteColliderPart;

/// The collision box node itself — dragging inside it moves the box.
#[derive(Component)]
struct PaletteColliderRect;

/// One of the box's eight resize handles. Index in atlas (y-down) order:
/// 0 NW, 1 N, 2 NE, 3 W, 4 E, 5 SW, 6 S, 7 SE.
#[derive(Component)]
struct PaletteColliderHandle(u8);

/// Wrapper node around the collision-shape dropdown in the header — shown only
/// while the current object pick has a collision box to give a shape to.
#[derive(Component)]
struct ShapePickWrap;

/// Dropdown labels for [`ShapePickWrap`], index-matched to
/// `CollisionShapeType::{Box, Sphere, Capsule}` (Sphere = a 2D circle).
const SHAPE_LABELS: [&str; 3] = ["Box", "Circle", "Capsule"];

/// Zoom step buttons.
#[derive(Component)]
struct ZoomInBtn;
#[derive(Component)]
struct ZoomOutBtn;

/// Accent colour for the selection highlight + handle.
const ACCENT: Color = Color::srgb(1.0, 0.6, 0.1);

/// Tint for solid-marked (collision) palette cells — red, clearly distinct
/// from the orange selection accent.
const SOLID_TINT: Color = Color::srgb(1.0, 0.25, 0.25);

/// The object collision box — green, matching the viewport's collider-edit
/// frame so it reads as the same concept in both places.
const COLLIDER_TINT: Color = Color::srgb(0.35, 0.9, 0.45);

/// Square size of a collision-box resize handle, in panel pixels.
const COLLIDER_HANDLE_PX: f32 = 10.0;

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
    app.init_resource::<TilemapList>();
    app.init_resource::<PaintLayerList>();
    app.register_shell_panel("tilemap", "Tilemap", "grid-four", "2D");
    app.register_panel_content("tilemap", false, build);
    app.add_systems(
        Update,
        (
            mirror_tilemap_list,
            mirror_paint_layer_list,
            paint_layer_clicks,
            add_paint_layer_click,
            tilemap_tab_clicks,
            tileset_drop_indicator,
            zoom_pan_atlas,
            zoom_step_buttons,
            apply_atlas_zoom,
            update_tile_highlight,
            update_cell_marks,
            update_solid_marks,
            toggle_solid_cells,
            sync_palette_collider_chrome,
            sync_shape_pick_visibility,
            palette_collider_drag,
            sync_grid_overlay,
            select_tiles_from_atlas,
            resize_selection,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Keep [`TilemapList`] mirroring the scene's `TilemapLayer` entities. Only
/// writes on an actual change so the reactive tab strip doesn't churn.
fn mirror_tilemap_list(
    layers: Query<(Entity, Option<&Name>), With<TilemapLayer>>,
    mut list: ResMut<TilemapList>,
) {
    let mut rows: Vec<(Entity, String)> = layers
        .iter()
        .map(|(e, n)| {
            let name = n
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| "Tilemap".to_string());
            (e, name)
        })
        .collect();
    rows.sort_by_key(|(e, _)| *e);
    if list.0 != rows {
        list.0 = rows;
    }
}

/// Clicking a tab makes its tilemap the active one; clicking the ACTIVE tab
/// again deselects it (which also disarms the brush — see
/// `sync_active_tilemap`), so the user can always put the panel down.
fn tilemap_tab_clicks(
    tabs: Query<(&Interaction, &TilemapTab), Changed<Interaction>>,
    mut active: ResMut<ActiveTilemap>,
) {
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed {
            active.0 = if active.0 == Some(tab.0) {
                None
            } else {
                Some(tab.0)
            };
        }
    }
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

    // Tab strip: one tab per tilemap in the scene (rebuilt reactively).
    let tabs = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, tabs, tabs_snapshot);

    // Paint-layer list (Godot-style, top row draws on top): eye / lock / name
    // / reorder per row, plus the "+ add" row. Rebuilt reactively.
    let layer_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, layer_list, paint_layers_snapshot);

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

    // Collision marking: single-cell picks toggle `solid_tiles`; object picks
    // toggle an editable collision box.
    let mark_solid = icon_button(commands, fonts, "wall");
    commands.entity(mark_solid).insert((
        MarkSolidBtn,
        HoverTooltip::new("Toggle collision on the selected tiles / object"),
    ));

    // Shape of the object collision box (Box / Circle / Capsule). Wrapped so
    // `sync_shape_pick_visibility` can hide it when the pick has no box.
    let shape_wrap = commands
        .spawn((
            Node {
                display: Display::None,
                ..default()
            },
            ShapePickWrap,
        ))
        .id();
    let shape_dd = dropdown(commands, fonts, &SHAPE_LABELS, 0);
    bind_2way(
        commands,
        shape_dd,
        |world: &World| -> usize {
            current_pick_collider(world)
                .map(|c| match c.shape {
                    renzora_physics::CollisionShapeType::Sphere => 1,
                    renzora_physics::CollisionShapeType::Capsule => 2,
                    _ => 0,
                })
                .unwrap_or(0)
        },
        |world: &mut World, idx: &usize| {
            let shape = match idx {
                1 => renzora_physics::CollisionShapeType::Sphere,
                2 => renzora_physics::CollisionShapeType::Capsule,
                _ => renzora_physics::CollisionShapeType::Box,
            };
            set_current_pick_shape(world, shape);
        },
    );
    commands.entity(shape_wrap).add_child(shape_dd);

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

    commands.entity(header).add_children(&[
        grid_icon, grid_switch, mark_solid, shape_wrap, spacer, zoom_out, zoom_dd, zoom_in,
    ]);

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

    // Drop indicator: an accent border + hint covering the whole panel,
    // toggled by `tileset_drop_indicator` while a tileset drag hovers. It
    // ignores picking so it never steals the drop's hover from the root.
    let drop_overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BorderColor::all(rgb(accent())),
            BackgroundColor(rgb(accent()).with_alpha(0.08)),
            bevy::picking::Pickable::IGNORE,
            TilesetDropIndicator,
        ))
        .id();
    let drop_hint = commands
        .spawn((
            Text::new("Drop tileset to import"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    commands.entity(drop_overlay).add_child(drop_hint);

    commands
        .entity(root)
        .add_children(&[tabs, layer_list, header, scroll_box, drop_overlay]);
    root
}

/// Show the drop indicator while a detached drag carrying at least one tileset
/// image hovers the panel.
fn tileset_drop_indicator(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    root: Query<&RelativeCursorPosition, With<TilemapPanelRoot>>,
    mut overlays: Query<&mut Node, With<TilesetDropIndicator>>,
) {
    let want = payload.as_ref().is_some_and(|p| {
        p.is_detached
            && root.iter().any(|r| r.cursor_over)
            && p.paths.iter().any(|path| crate::is_tileset(path))
    });
    for mut node in &mut overlays {
        let display = if want { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

/// One tab per tilemap, keyed on entity + name + whether it's active (so a
/// rename or an active-tilemap switch restyles just the affected tabs).
fn tabs_snapshot(world: &World) -> KeyedSnapshot {
    let active = world.get_resource::<ActiveTilemap>().and_then(|a| a.0);
    let rows = world
        .get_resource::<TilemapList>()
        .map(|l| l.0.clone())
        .unwrap_or_default();

    let mut items = Vec::with_capacity(rows.len());
    for (e, name) in &rows {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        (active == Some(*e)).hash(&mut hasher);
        items.push((e.to_bits(), hasher.finish()));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, fonts: &EmberFonts, i: usize| {
            let (entity, name) = rows[i].clone();
            let is_active = active == Some(entity);
            let tab = c
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(if is_active { rgb(tab_active()) } else { Color::NONE }),
                    Interaction::default(),
                    TilemapTab(entity),
                ))
                .id();
            let text = c
                .spawn((
                    Text::new(name),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(if is_active {
                        rgb(text_primary())
                    } else {
                        rgb(text_muted())
                    }),
                    bevy::picking::Pickable::IGNORE,
                ))
                .id();
            c.entity(tab).add_child(text);
            tab
        }),
    }
}

/// Keep [`PaintLayerList`] mirroring the ACTIVE tilemap's paint layers, topmost
/// (highest `order`) first, with the implicit **Base** row (the tilemap root)
/// last. Also drops [`ActivePaintLayer`] back to Base when its layer vanished
/// or belongs to another tilemap, so strokes never land somewhere invisible.
fn mirror_paint_layer_list(
    active: Res<ActiveTilemap>,
    mut active_layer: ResMut<ActivePaintLayer>,
    layers: Query<(Entity, &TilemapPaintLayer, &ChildOf, Option<&Name>, &Visibility)>,
    mut list: ResMut<PaintLayerList>,
) {
    let mut rows: Vec<PaintLayerRowData> = Vec::new();
    if let Some(root) = active.0 {
        for (e, pl, child_of, name, vis) in &layers {
            if child_of.parent() != root {
                continue;
            }
            rows.push(PaintLayerRowData {
                entity: Some(e),
                name: name
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| "Layer".to_string()),
                order: pl.order,
                visible: *vis != Visibility::Hidden,
                locked: pl.locked,
            });
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.order));
        rows.push(PaintLayerRowData {
            entity: None,
            name: "Base".to_string(),
            order: i32::MIN,
            visible: true,
            locked: false,
        });
    }
    if let Some(sel) = active_layer.0 {
        if !rows.iter().any(|r| r.entity == Some(sel)) {
            active_layer.0 = None;
        }
    }
    if list.0 != rows {
        list.0 = rows;
    }
}

/// Rows of the paint-layer list + the "+ add" row. Keyed on each row's full
/// state so a rename/toggle/reorder restyles only the affected rows.
fn paint_layers_snapshot(world: &World) -> KeyedSnapshot {
    let rows = world
        .get_resource::<PaintLayerList>()
        .map(|l| l.0.clone())
        .unwrap_or_default();
    let selected = world
        .get_resource::<ActivePaintLayer>()
        .and_then(|a| a.0);
    let has_tilemap = !rows.is_empty();

    let mut items = Vec::with_capacity(rows.len() + 1);
    for r in &rows {
        let mut hasher = DefaultHasher::new();
        r.name.hash(&mut hasher);
        r.order.hash(&mut hasher);
        r.visible.hash(&mut hasher);
        r.locked.hash(&mut hasher);
        (selected == r.entity).hash(&mut hasher);
        items.push((
            r.entity.map(|e| e.to_bits()).unwrap_or(u64::MAX - 1),
            hasher.finish(),
        ));
    }
    if has_tilemap {
        items.push((u64::MAX, 0));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, fonts: &EmberFonts, i: usize| {
            // Last item = the "+ add layer" row.
            if i == rows.len() {
                let btn = commands_add_layer_row(c, fonts);
                return btn;
            }
            let r = rows[i].clone();
            let is_active = selected == r.entity;
            let row = c
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(if is_active { rgb(tab_active()) } else { Color::NONE }),
                    Interaction::default(),
                    PaintLayerRow(r.entity),
                ))
                .id();
            // Base has no eye/lock/reorder — it's the always-there fallback.
            if let Some(e) = r.entity {
                let eye = icon_text(
                    c,
                    &fonts.phosphor,
                    if r.visible { "eye" } else { "eye-slash" },
                    if r.visible { text_primary() } else { text_muted() },
                    13.0,
                );
                c.entity(eye).insert((Interaction::default(), PaintLayerEye(e)));
                let lock = icon_text(
                    c,
                    &fonts.phosphor,
                    if r.locked { "lock" } else { "lock-open" },
                    if r.locked { text_primary() } else { text_muted() },
                    13.0,
                );
                c.entity(lock).insert((Interaction::default(), PaintLayerLock(e)));
                c.entity(row).add_children(&[eye, lock]);
            } else {
                let glyph = icon_text(c, &fonts.phosphor, "stack", text_muted(), 13.0);
                c.entity(row).add_child(glyph);
            }
            let label = c
                .spawn((
                    Text::new(r.name.clone()),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(if is_active {
                        rgb(text_primary())
                    } else {
                        rgb(text_muted())
                    }),
                    bevy::picking::Pickable::IGNORE,
                ))
                .id();
            let spacer = c.spawn(Node { flex_grow: 1.0, ..default() }).id();
            c.entity(row).add_children(&[label, spacer]);
            if let Some(e) = r.entity {
                for up in [true, false] {
                    let arrow = icon_text(
                        c,
                        &fonts.phosphor,
                        if up { "caret-up" } else { "caret-down" },
                        text_muted(),
                        13.0,
                    );
                    c.entity(arrow)
                        .insert((Interaction::default(), PaintLayerReorder { layer: e, up }));
                    c.entity(row).add_child(arrow);
                }
            }
            row
        }),
    }
}

/// The "+ add layer" row under the list.
fn commands_add_layer_row(c: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = c
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            Interaction::default(),
            AddPaintLayerBtn,
            HoverTooltip::new("Add a paint layer"),
        ))
        .id();
    let plus = icon_text(c, &fonts.phosphor, "plus", text_muted(), 12.0);
    let label = c
        .spawn((
            Text::new("Add Layer"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    c.entity(row).add_children(&[plus, label]);
    row
}

/// Row / eye / lock / reorder clicks in the paint-layer list.
fn paint_layer_clicks(
    rows: Query<(&Interaction, &PaintLayerRow), Changed<Interaction>>,
    eyes: Query<(&Interaction, &PaintLayerEye), Changed<Interaction>>,
    locks: Query<(&Interaction, &PaintLayerLock), Changed<Interaction>>,
    reorders: Query<(&Interaction, &PaintLayerReorder), Changed<Interaction>>,
    mut active_layer: ResMut<ActivePaintLayer>,
    mut vis: Query<&mut Visibility, With<TilemapPaintLayer>>,
    mut layers: Query<&mut TilemapPaintLayer>,
) {
    // Eye / lock / reorder first — they sit inside the row, and their press
    // must not double as a row-select.
    let mut consumed = false;
    for (i, eye) in &eyes {
        if *i == Interaction::Pressed {
            if let Ok(mut v) = vis.get_mut(eye.0) {
                *v = if *v == Visibility::Hidden {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            consumed = true;
        }
    }
    for (i, lock) in &locks {
        if *i == Interaction::Pressed {
            if let Ok(mut l) = layers.get_mut(lock.0) {
                l.locked = !l.locked;
            }
            consumed = true;
        }
    }
    for (i, re) in &reorders {
        if *i == Interaction::Pressed {
            if let Ok(mut l) = layers.get_mut(re.layer) {
                // Draw order is plain integer nudging — collisions between two
                // layers at the same order are harmless (stable z tie) and the
                // next nudge separates them again. Simpler than swap logic.
                l.order += if re.up { 1 } else { -1 };
            }
            consumed = true;
        }
    }
    if consumed {
        return;
    }
    for (i, row) in &rows {
        if *i == Interaction::Pressed && active_layer.0 != row.0 {
            active_layer.0 = row.0;
        }
    }
}

/// "+ add layer": spawn a `TilemapPaintLayer` child of the active tilemap, on
/// top of the current stack, and make it the paint target.
fn add_paint_layer_click(
    buttons: Query<&Interaction, (With<AddPaintLayerBtn>, Changed<Interaction>)>,
    active: Res<ActiveTilemap>,
    mut active_layer: ResMut<ActivePaintLayer>,
    existing: Query<(&TilemapPaintLayer, &ChildOf)>,
    mut commands: Commands,
) {
    if !buttons.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(root) = active.0 else { return };
    let (top_order, count) = existing
        .iter()
        .filter(|(_, c)| c.parent() == root)
        .fold((0, 0), |(top, n), (pl, _)| (top.max(pl.order), n + 1));
    let order = if count == 0 { 1 } else { top_order + 1 };
    let layer = commands
        .spawn((
            Name::new(format!("Layer {}", count + 1)),
            renzora::core::Node2d,
            TilemapPaintLayer {
                order,
                ..Default::default()
            },
            Transform::from_xyz(0.0, 0.0, order as f32 * PAINT_LAYER_Z_STEP),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    active_layer.0 = Some(layer);
}

/// Single-item snapshot: the active tilemap's atlas image + its selection
/// highlight (with resize handle), keyed on tileset path + dimensions. When no
/// tilemap exists yet, the item is instead a hint that dropping a tileset here
/// imports one. The grid overlay is intentionally NOT part of this — see
/// [`sync_grid_overlay`].
fn atlas_snapshot(world: &World) -> KeyedSnapshot {
    let mut items = Vec::new();
    let mut data: Option<(Handle<Image>, Vec2)> = None;

    let active = world.get_resource::<ActiveTilemap>().and_then(|a| a.0);
    if let Some(e) = active {
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
    } else {
        // No active tilemap. Distinguish "nothing imported yet" (the panel is
        // the import surface, say so) from "tilemaps exist but none picked".
        let any_tilemaps = world
            .get_resource::<TilemapList>()
            .is_some_and(|l| !l.0.is_empty());
        items.push((2u64, any_tilemaps as u64));
    }
    let no_active_hint = if world
        .get_resource::<TilemapList>()
        .is_some_and(|l| !l.0.is_empty())
    {
        "Select a tilemap above to pick a brush"
    } else {
        "Drop a tileset image here to create a tilemap"
    };

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, fonts: &EmberFonts, _i: usize| {
            let Some((handle, px)) = data.clone() else {
                return label(c, &fonts.ui, no_active_hint);
            };
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
    active: Res<ActiveTilemap>,
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
    let Some(e) = active.0 else { return };
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
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    tilesets: Query<&TilesetHandle>,
    mut image_nodes: Query<&mut Node, With<TilesetImageNode>>,
) {
    let Ok(mut node) = image_nodes.single_mut() else {
        return;
    };
    let Some(e) = active.0 else { return };
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
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    mut highlight: Query<&mut Node, With<TileHighlight>>,
) {
    let Ok(mut node) = highlight.single_mut() else {
        return;
    };
    // A multi-tile pick is shown by a border PER cell (`update_cell_marks`) so
    // every selected tile reads as its own selection; the single enclosing box
    // is only right for a lone cell, so hide it once more than one is picked.
    let want_display = if brush.selected.len() >= 2 {
        Display::None
    } else {
        Display::Flex
    };
    if node.display != want_display {
        node.display = want_display;
    }
    if want_display == Display::None {
        return;
    }
    let Some(e) = active.0 else { return };
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

/// Tint each individually-picked atlas cell so a **non-rectangular** selection
/// (a tree's canopy over a narrow trunk, built with Ctrl/Shift+click) shows
/// exactly which cells are in — the bounding-box highlight alone can't show
/// holes. A plain solid-rectangle pick draws none (the box already conveys it).
/// Rebuilds only when the pick or atlas changes.
#[allow(clippy::too_many_arguments)]
fn update_cell_marks(
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas_node: Query<Entity, With<TilesetImageNode>>,
    marks: Query<Entity, With<TileCellMark>>,
    mut commands: Commands,
    mut built: Local<u64>,
) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let dims = active.0.and_then(|e| layers.get(e).ok()).and_then(|(layer, tileset)| {
        let s = images.get(&tileset.image)?.size_f32();
        let cols = layer.effective_columns(s.x).max(1);
        let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
        Some((cols, rows))
    });
    let Some((cols, rows)) = dims else {
        if !marks.is_empty() {
            for m in &marks {
                commands.entity(m).try_despawn();
            }
            *built = 0;
        }
        return;
    };

    // A per-cell border is drawn for every multi-tile pick (solid or scattered)
    // so each selected tile reads as its own selection. A lone cell keeps the
    // single bounding-box highlight (with its resize handle) instead.
    let multi = brush.selected.len() >= 2;
    let mut hasher = DefaultHasher::new();
    (multi, cols, rows).hash(&mut hasher);
    for c in &brush.selected {
        (c.x, c.y).hash(&mut hasher);
    }
    let key = hasher.finish();
    if *built == key {
        return;
    }
    *built = key;

    for m in &marks {
        commands.entity(m).try_despawn();
    }
    if !multi {
        return;
    }
    let Ok(atlas_e) = atlas_node.single() else {
        return;
    };
    for c in &brush.selected {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(c.x as f32 / cols as f32 * 100.0),
                top: Val::Percent(c.y as f32 / rows as f32 * 100.0),
                width: Val::Percent(100.0 / cols as f32),
                height: Val::Percent(100.0 / rows as f32),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            // A border per picked cell (plus a faint fill) so a scattered pick
            // reads as several distinct selections, not one enclosing box.
            BackgroundColor(ACCENT.with_alpha(0.18)),
            BorderColor::all(ACCENT),
            TileCellMark,
            bevy::picking::Pickable::IGNORE,
            ChildOf(atlas_e),
        ));
    }
}

/// The header's wall button — one collision toggle, two meanings by pick size:
///
/// - **Single cell:** toggle membership in `TilemapLayer.solid_tiles`. If every
///   picked cell is already solid the click clears them; otherwise it marks
///   them all. The runtime's `rebuild_tile_colliders` grows the merged tile
///   colliders from this set.
/// - **Multi-cell (object) pick:** toggle an editable **collision box** for
///   this pick's atlas bounding box (`TilemapLayer.object_colliders`). It
///   appears as a green rect over the selection — drag its handles/body to
///   shape it ([`palette_collider_drag`]) — and every object stamped from this
///   pick carries it as a `CollisionShapeData`.
fn toggle_solid_cells(
    buttons: Query<&Interaction, (With<MarkSolidBtn>, Changed<Interaction>)>,
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    tilesets: Query<&TilesetHandle>,
    mut layers: Query<&mut TilemapLayer>,
) {
    if !buttons.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(e) = active.0 else { return };
    let Ok(tileset) = tilesets.get(e) else { return };
    let Ok(mut layer) = layers.get_mut(e) else { return };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let cols = layer.effective_columns(img.size_f32().x).max(1);

    // Object pick → toggle its collision box (default: the full footprint,
    // ready to be dragged down to a trunk).
    if brush.selected.len() > 1 {
        let (c, r, w, h) = (brush.col, brush.row, brush.w.max(1), brush.h.max(1));
        if let Some(i) = layer
            .object_colliders
            .iter()
            .position(|k| k.col == c && k.row == r && k.w == w && k.h == h)
        {
            layer.object_colliders.remove(i);
        } else {
            layer.object_colliders.push(TileObjectCollider {
                col: c,
                row: r,
                w,
                h,
                rect_x: 0.0,
                rect_y: 0.0,
                rect_w: w as f32,
                rect_h: h as f32,
                shape: Default::default(),
            });
        }
        return;
    }

    let indices: Vec<u32> = brush.selected.iter().map(|c| c.y * cols + c.x).collect();
    if indices.is_empty() {
        return;
    }
    if indices.iter().all(|i| layer.solid_tiles.contains(i)) {
        layer.solid_tiles.retain(|i| !indices.contains(i));
    } else {
        for i in indices {
            if !layer.solid_tiles.contains(&i) {
                layer.solid_tiles.push(i);
            }
        }
    }
}

/// The authored collision box for the CURRENT palette pick, if any — the
/// `&World` form the reactive dropdown bindings need.
fn current_pick_collider(world: &World) -> Option<TileObjectCollider> {
    let active = world.get_resource::<ActiveTilemap>().and_then(|a| a.0)?;
    let brush = world.get_resource::<TilemapBrush>()?;
    if brush.selected.len() < 2 {
        return None;
    }
    world
        .get::<TilemapLayer>(active)?
        .collider_for(brush.col, brush.row, brush.w.max(1), brush.h.max(1))
}

/// Write a new shape onto the current pick's collision box (dropdown setter).
fn set_current_pick_shape(world: &mut World, shape: renzora_physics::CollisionShapeType) {
    let Some(active) = world.get_resource::<ActiveTilemap>().and_then(|a| a.0) else {
        return;
    };
    let Some((c, r, w, h)) = world
        .get_resource::<TilemapBrush>()
        .map(|b| (b.col, b.row, b.w.max(1), b.h.max(1)))
    else {
        return;
    };
    let Some(mut layer) = world.get_mut::<TilemapLayer>(active) else {
        return;
    };
    if let Some(k) = layer
        .object_colliders
        .iter_mut()
        .find(|k| k.col == c && k.row == r && k.w == w && k.h == h)
    {
        if k.shape != shape {
            k.shape = shape;
        }
    }
}

/// Show the shape dropdown only while the current pick has a collision box.
fn sync_shape_pick_visibility(
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    layers: Query<&TilemapLayer>,
    mut wraps: Query<&mut Node, With<ShapePickWrap>>,
) {
    let want = active
        .0
        .and_then(|e| layers.get(e).ok())
        .is_some_and(|layer| {
            brush.selected.len() > 1
                && layer
                    .collider_for(brush.col, brush.row, brush.w.max(1), brush.h.max(1))
                    .is_some()
        });
    let display = if want { Display::Flex } else { Display::None };
    for mut node in &mut wraps {
        if node.display != display {
            node.display = display;
        }
    }
}

/// Show/update the green collision box over the current object pick, when one
/// is authored ([`toggle_solid_cells`]). The rect is a child of the atlas
/// image positioned in **percent**, so it rides zoom for free; its eight
/// handles are children of the rect at percent corners (pixel-size squares via
/// negative margins), so they ride the rect. Geometry is written in place each
/// frame — the nodes must stay STABLE across frames or an in-flight handle
/// drag would lose its `Interaction` mid-gesture.
fn sync_palette_collider_chrome(
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas_node: Query<Entity, With<TilesetImageNode>>,
    parts: Query<Entity, With<PaletteColliderPart>>,
    mut rects: Query<&mut Node, With<PaletteColliderRect>>,
    mut commands: Commands,
) {
    let data = active.0.and_then(|e| layers.get(e).ok()).and_then(|(layer, tileset)| {
        if brush.selected.len() < 2 {
            return None;
        }
        let s = images.get(&tileset.image)?.size_f32();
        let cols = layer.effective_columns(s.x).max(1);
        let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
        let collider =
            layer.collider_for(brush.col, brush.row, brush.w.max(1), brush.h.max(1))?;
        Some((collider, cols, rows))
    });
    let Some((collider, cols, rows)) = data else {
        for p in &parts {
            commands.entity(p).try_despawn();
        }
        return;
    };

    let left = Val::Percent((collider.col as f32 + collider.rect_x) / cols as f32 * 100.0);
    let top = Val::Percent((collider.row as f32 + collider.rect_y) / rows as f32 * 100.0);
    let width = Val::Percent(collider.rect_w / cols as f32 * 100.0);
    let height = Val::Percent(collider.rect_h / rows as f32 * 100.0);
    // Round the frame to the shape it stamps: percent border radius resolves
    // against the node's shorter side, so 50% is a true circle on a square
    // rect and proper end caps on a tall capsule — matching how `shape_data`
    // derives the radius from the shorter dimension.
    let corner = match collider.shape {
        renzora_physics::CollisionShapeType::Sphere
        | renzora_physics::CollisionShapeType::Capsule => Val::Percent(50.0),
        _ => Val::Px(0.0),
    };
    let radius = BorderRadius::all(corner);

    if let Ok(mut node) = rects.single_mut() {
        if node.left != left {
            node.left = left;
        }
        if node.top != top {
            node.top = top;
        }
        if node.width != width {
            node.width = width;
        }
        if node.height != height {
            node.height = height;
        }
        if node.border_radius != radius {
            node.border_radius = radius;
        }
        return;
    }

    // Nothing on screen yet (fresh toggle, or the atlas image was rebuilt and
    // took the chrome with it) — spawn the rect + handles.
    let Ok(atlas_e) = atlas_node.single() else {
        return;
    };
    let rect = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left,
                top,
                width,
                height,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: radius,
                ..default()
            },
            BackgroundColor(COLLIDER_TINT.with_alpha(0.15)),
            BorderColor::all(COLLIDER_TINT),
            Interaction::default(),
            PaletteColliderPart,
            PaletteColliderRect,
            ChildOf(atlas_e),
            Name::new("palette-collider-rect"),
        ))
        .id();
    // Percent-of-rect corner/edge anchors, pulled back by half the handle size.
    let anchors: [(f32, f32); 8] = [
        (0.0, 0.0),
        (50.0, 0.0),
        (100.0, 0.0),
        (0.0, 50.0),
        (100.0, 50.0),
        (0.0, 100.0),
        (50.0, 100.0),
        (100.0, 100.0),
    ];
    for (i, (px, py)) in anchors.into_iter().enumerate() {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(px),
                top: Val::Percent(py),
                margin: UiRect {
                    left: Val::Px(-COLLIDER_HANDLE_PX * 0.5),
                    top: Val::Px(-COLLIDER_HANDLE_PX * 0.5),
                    ..default()
                },
                width: Val::Px(COLLIDER_HANDLE_PX),
                height: Val::Px(COLLIDER_HANDLE_PX),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            BorderColor::all(COLLIDER_TINT),
            Interaction::default(),
            PaletteColliderPart,
            PaletteColliderHandle(i as u8),
            ChildOf(rect),
            Name::new("palette-collider-handle"),
        ));
    }
}

/// Drag the collision box's handles (resize) or body (move), writing straight
/// into the layer's `object_colliders` entry — [`sync_palette_collider_chrome`]
/// reflects it on screen the same frame, and the next stamped object picks up
/// the new shape. Everything works in continuous CELL coordinates relative to
/// the pick's bounding box (the unit `TileObjectCollider` stores), clamped to
/// the footprint.
fn palette_collider_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    atlas: Query<&RelativeCursorPosition, With<TilesetImageNode>>,
    handles: Query<(&Interaction, &PaletteColliderHandle)>,
    rect_node: Query<&Interaction, With<PaletteColliderRect>>,
    mut layers: Query<(&mut TilemapLayer, &TilesetHandle)>,
    // (grabbed handle — None = body move, collider at press, cursor at press)
    mut drag: Local<Option<(Option<u8>, TileObjectCollider, Vec2)>>,
) {
    if !mouse.pressed(MouseButton::Left) {
        *drag = None;
        return;
    }
    let Some(e) = active.0 else {
        *drag = None;
        return;
    };
    let Ok((mut layer, tileset)) = layers.get_mut(e) else {
        return;
    };
    let Some(img) = images.get(&tileset.image) else {
        return;
    };
    let s = img.size_f32();
    let cols = layer.effective_columns(s.x).max(1);
    let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
    let Ok(rcp) = atlas.single() else { return };
    let Some(n) = rcp.normalized else { return };
    let (bc, br, bw, bh) = (brush.col, brush.row, brush.w.max(1), brush.h.max(1));
    // Continuous cell position relative to the pick's bounding-box top-left.
    let local = Vec2::new(
        (n.x + 0.5) * cols as f32 - bc as f32,
        (n.y + 0.5) * rows as f32 - br as f32,
    );
    let Some(idx) = layer
        .object_colliders
        .iter()
        .position(|k| k.col == bc && k.row == br && k.w == bw && k.h == bh)
    else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        let start = layer.object_colliders[idx];
        let grabbed_handle = handles
            .iter()
            .find(|(i, _)| **i == Interaction::Pressed)
            .map(|(_, h)| h.0);
        if let Some(h) = grabbed_handle {
            *drag = Some((Some(h), start, local));
        } else if rect_node.iter().any(|i| *i == Interaction::Pressed) {
            *drag = Some((None, start, local));
        }
    }
    let Some((handle, start, grab)) = *drag else {
        return;
    };
    let (w, h) = (bw as f32, bh as f32);
    let c = &mut layer.object_colliders[idx];
    match handle {
        // Body move: keep the grab point pinned, clamp inside the footprint.
        None => {
            let d = local - grab;
            c.rect_x = (start.rect_x + d.x).clamp(0.0, (w - start.rect_w).max(0.0));
            c.rect_y = (start.rect_y + d.y).clamp(0.0, (h - start.rect_h).max(0.0));
        }
        // Handle resize: the grabbed edge(s) follow the cursor, the box may
        // flip past the opposite edge (min/max normalise), floored at 0.1 cell.
        Some(hi) => {
            let (mut x0, mut y0) = (start.rect_x, start.rect_y);
            let (mut x1, mut y1) = (start.rect_x + start.rect_w, start.rect_y + start.rect_h);
            let cx = local.x.clamp(0.0, w);
            let cy = local.y.clamp(0.0, h);
            match hi {
                0 => {
                    x0 = cx;
                    y0 = cy;
                }
                1 => y0 = cy,
                2 => {
                    x1 = cx;
                    y0 = cy;
                }
                3 => x0 = cx,
                4 => x1 = cx,
                5 => {
                    x0 = cx;
                    y1 = cy;
                }
                6 => y1 = cy,
                7 => {
                    x1 = cx;
                    y1 = cy;
                }
                _ => {}
            }
            const MIN_CELLS: f32 = 0.1;
            let (lo_x, hi_x) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
            let (lo_y, hi_y) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
            c.rect_x = lo_x;
            c.rect_y = lo_y;
            c.rect_w = (hi_x - lo_x).max(MIN_CELLS);
            c.rect_h = (hi_y - lo_y).max(MIN_CELLS);
        }
    }
}

/// Tint every solid-marked (collision) palette cell red, independent of the
/// selection overlays. Rebuilds only when the solid set, the atlas grid, or
/// the atlas image node itself changes — the node entity is part of the key
/// because switching tilemaps rebuilds the image (despawning these children
/// with it), which must not be mistaken for "already built".
fn update_solid_marks(
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas_node: Query<Entity, With<TilesetImageNode>>,
    marks: Query<Entity, With<SolidCellMark>>,
    mut commands: Commands,
    mut built: Local<u64>,
) {
    let dims = active.0.and_then(|e| layers.get(e).ok()).and_then(|(layer, tileset)| {
        let s = images.get(&tileset.image)?.size_f32();
        let cols = layer.effective_columns(s.x).max(1);
        let rows = ((s.y / layer.atlas_tile_px.max(1) as f32).floor() as u32).max(1);
        Some((layer.solid_tiles.clone(), cols, rows))
    });
    let Some((solid, cols, rows)) = dims else {
        if !marks.is_empty() {
            for m in &marks {
                commands.entity(m).try_despawn();
            }
            *built = 0;
        }
        return;
    };
    let Ok(atlas_e) = atlas_node.single() else {
        return;
    };

    let mut hasher = DefaultHasher::new();
    (atlas_e.to_bits(), cols, rows).hash(&mut hasher);
    let mut sorted = solid.clone();
    sorted.sort_unstable();
    sorted.hash(&mut hasher);
    let key = hasher.finish();
    if *built == key {
        return;
    }
    *built = key;

    for m in &marks {
        commands.entity(m).try_despawn();
    }
    for idx in &solid {
        let (cx, cy) = (idx % cols, idx / cols);
        if cy >= rows {
            continue; // stale index from a different/smaller atlas
        }
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(cx as f32 / cols as f32 * 100.0),
                top: Val::Percent(cy as f32 / rows as f32 * 100.0),
                width: Val::Percent(100.0 / cols as f32),
                height: Val::Percent(100.0 / rows as f32),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(SOLID_TINT.with_alpha(0.22)),
            BorderColor::all(SOLID_TINT.with_alpha(0.8)),
            SolidCellMark,
            bevy::picking::Pickable::IGNORE,
            ChildOf(atlas_e),
        ));
    }
}

/// Left-drag a rectangle across the atlas to select a block of tiles (a single
/// click selects one) — selecting also **arms the paint brush**, so the block
/// immediately rides the cursor in the 2D viewport. Skips scrollbar +
/// resize-handle interactions so grabbing those doesn't start a selection.
#[allow(clippy::too_many_arguments)]
fn select_tiles_from_atlas(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas: Query<&RelativeCursorPosition, With<TilesetImageNode>>,
    thumbs: Query<&Interaction, With<ScrollThumb>>,
    handles: Query<&Interaction, With<SelectionHandle>>,
    collider_parts: Query<&Interaction, With<PaletteColliderPart>>,
    scrollbar: Res<ScrollbarBusy>,
    mut brush: ResMut<TilemapBrush>,
    mut settings: Option<ResMut<ViewportSettings>>,
    mut start: Local<Option<(u32, u32)>>,
    mut blocked: Local<bool>,
    // While a Shift+drag is in flight, the selection this drag started from —
    // the dragged rectangle is UNIONed onto it so Shift+drag *adds* a block.
    // `None` = a plain (replace) drag.
    mut add_base: Local<Option<Vec<UVec2>>>,
) {
    // An asset drag passing over the atlas is a DROP gesture, not a tile
    // selection — the held left button would otherwise sweep the brush rect
    // (and arm the paint mode) while the user is just importing a tileset.
    if payload.is_some() {
        *start = None;
        *add_base = None;
        *blocked = true;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *start = None;
        *add_base = None;
        *blocked = false;
        return;
    }
    // On press, decide whether this drag belongs to a scrollbar, the resize
    // handle, or the collision-box editor — if so, don't select for the rest
    // of the hold. (A press on the collision box is a move/resize of it, not
    // a tile pick — and re-picking would change the bounding box out from
    // under the drag anyway.)
    if mouse.just_pressed(MouseButton::Left) {
        let on_bar = scrollbar.active()
            || thumbs
                .iter()
                .any(|i| matches!(i, Interaction::Pressed | Interaction::Hovered));
        let on_handle = handles.iter().any(|i| *i == Interaction::Pressed);
        let on_collider = collider_parts.iter().any(|i| *i == Interaction::Pressed);
        *blocked = on_bar || on_handle || on_collider;
    }
    if *blocked {
        return;
    }
    let Some(rcp) = atlas.iter().find(|r| r.cursor_over) else {
        return;
    };
    let Some(n) = rcp.normalized else { return };
    let Some(entity) = active.0 else { return };
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
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if mouse.just_pressed(MouseButton::Left) {
        // Picking tiles is the intent to paint — switch the viewport's Mode
        // dropdown to Paint right away (Esc or Tab drops back to Scene; the
        // dropdown is the mode's single source of truth).
        if let Some(settings) = settings.as_deref_mut() {
            if settings.viewport_mode != ViewportMode::Paint {
                settings.viewport_mode = ViewportMode::Paint;
            }
        }
        // Ctrl+click toggles a single cell in/out — a click-only gesture to
        // punch a hole or add one stray tile, so `start` stays cleared.
        if ctrl {
            brush.toggle(cur.0, cur.1, cols);
            *start = None;
            *add_base = None;
            return;
        }
        // A drag anchors here. Shift makes it ADD to the current pick (grab a
        // tree's branches as a second block), snapshotting the pre-drag set;
        // a plain drag REPLACES.
        *start = Some(cur);
        *add_base = shift.then(|| brush.selected.clone());
    }
    let Some(s) = *start else { return };
    if let Some(base) = add_base.as_ref() {
        // Shift+drag: union the dragged rectangle onto the snapshot.
        let mut sel = base.clone();
        for r in s.1.min(cur.1)..=s.1.max(cur.1) {
            for c in s.0.min(cur.0)..=s.0.max(cur.0) {
                let cell = UVec2::new(c, r);
                if !sel.contains(&cell) {
                    sel.push(cell);
                }
            }
        }
        brush.set_cells(sel, cols);
    } else {
        // Plain click/drag selects a solid rectangle (replace).
        brush.set_rect(s.0, s.1, cur.0, cur.1, cols);
    }
}

/// Drag the corner handle to grow the selection from its fixed top-left corner.
fn resize_selection(
    handles: Query<&Interaction, With<SelectionHandle>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    active: Res<ActiveTilemap>,
    images: Res<Assets<Image>>,
    layers: Query<(&TilemapLayer, &TilesetHandle)>,
    atlas: Query<&RelativeCursorPosition, With<TilesetImageNode>>,
    mut brush: ResMut<TilemapBrush>,
) {
    if payload.is_some() || !handles.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Ok(rcp) = atlas.single() else { return };
    let Some(n) = rcp.normalized else { return };
    let Some(e) = active.0 else { return };
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
    // Dragging the corner handle re-selects a solid rectangle from the pick's
    // top-left to the cursor (a rectangle gesture drops any scattered picks).
    let (c0, r0) = (brush.col, brush.row);
    brush.set_rect(c0, r0, cur_col.max(c0), cur_row.max(r0), cols);
}
