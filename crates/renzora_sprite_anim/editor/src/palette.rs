//! The sheet **palette**: zoom/pan the current sprite sheet, overlay its grid,
//! and select cells into an ordered [`SheetSelection`]. Ported and trimmed from
//! the Tilemap palette (`renzora_tilemap_editor`) — the interaction is the same
//! (wheel zoom toward cursor, right-drag pan, left-drag marquee, Ctrl+click),
//! but selecting here builds a frame list instead of arming a paint brush.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, ScrollPosition};

use renzora::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, rgb};
use renzora_ember::widgets::{EmberScroll, ScrollThumb, ScrollbarBusy};

use crate::panel::{GridLine, SelCellMark, SheetImageNode, ZoomInBtn, ZoomOutBtn};
use crate::{CurrentSheet, SheetGrid, SheetSelection, SheetZoom};

/// Accent colour for cell selection marks.
fn accent_color() -> Color {
    rgb(accent())
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            zoom_step_buttons,
            zoom_pan_sheet,
            apply_sheet_zoom,
            sync_grid_overlay,
            select_cells_from_sheet,
            update_cell_marks,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// The `−` / `+` buttons step the zoom.
fn zoom_step_buttons(
    zoom_in: Query<&Interaction, (With<ZoomInBtn>, Changed<Interaction>)>,
    zoom_out: Query<&Interaction, (With<ZoomOutBtn>, Changed<Interaction>)>,
    mut zoom: ResMut<SheetZoom>,
) {
    if zoom_in.iter().any(|i| *i == Interaction::Pressed) {
        zoom.0 = (zoom.0 * 1.25).clamp(0.25, 32.0);
    }
    if zoom_out.iter().any(|i| *i == Interaction::Pressed) {
        zoom.0 = (zoom.0 / 1.25).clamp(0.25, 32.0);
    }
}

/// Wheel zooms toward the cursor; right-drag pans. Drives the ember scroll
/// viewport's offset (found by walking image → content → viewport).
fn zoom_pan_sheet(
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    mut zoom: ResMut<SheetZoom>,
    sheet: Query<Entity, With<SheetImageNode>>,
    parents: Query<&ChildOf>,
    mut viewports: Query<(&mut EmberScroll, &mut ScrollPosition, &ComputedNode, &RelativeCursorPosition)>,
) {
    let Ok(img) = sheet.single() else {
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
            let new = (old * 1.15f32.powf(wy)).clamp(0.25, 32.0);
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

/// Size the sheet image from the zoom (native × zoom). Reads the loaded image's
/// size via the `ImageNode`'s own handle.
fn apply_sheet_zoom(
    zoom: Res<SheetZoom>,
    images: Res<Assets<Image>>,
    mut nodes: Query<(&mut Node, &ImageNode), With<SheetImageNode>>,
) {
    let Ok((mut node, image_node)) = nodes.single_mut() else { return };
    let Some(img) = images.get(&image_node.image) else { return };
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

/// (Re)build the grid overlay as children of the sheet image whenever the image
/// or grid changes. Percent positions so lines track zoom for free.
fn sync_grid_overlay(
    grid: Res<SheetGrid>,
    image_node: Query<(Entity, &ImageNode), With<SheetImageNode>>,
    lines: Query<Entity, With<GridLine>>,
    mut commands: Commands,
    mut built: Local<u64>,
) {
    let Ok((img, node)) = image_node.single() else {
        // No sheet — clear any stale lines.
        if !lines.is_empty() {
            for l in &lines {
                commands.entity(l).try_despawn();
            }
            *built = 0;
        }
        return;
    };
    let mut hasher = DefaultHasher::new();
    (img.to_bits(), grid.cols, grid.rows, node.image.id()).hash(&mut hasher);
    let key = hasher.finish();
    if *built == key {
        return;
    }
    *built = key;

    for l in &lines {
        commands.entity(l).try_despawn();
    }
    let (cols, rows) = (grid.cols.max(1), grid.rows.max(1));
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

/// Left-drag a rectangle to select cells (row-major order — a single column
/// reads top→bottom, a single row left→right); Ctrl+click appends/removes a
/// single cell in click order. Right-drag pans, so it's ignored here. Grabbing a
/// scrollbar doesn't start a selection.
#[allow(clippy::too_many_arguments)]
fn select_cells_from_sheet(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    grid: Res<SheetGrid>,
    sheet: Res<CurrentSheet>,
    image: Query<&RelativeCursorPosition, With<SheetImageNode>>,
    thumbs: Query<&Interaction, With<ScrollThumb>>,
    scrollbar: Res<ScrollbarBusy>,
    mut selection: ResMut<SheetSelection>,
    mut start: Local<Option<UVec2>>,
    mut blocked: Local<bool>,
) {
    // An asset drag over the sheet is a DROP, not a selection.
    if payload.is_some() {
        *start = None;
        *blocked = true;
        return;
    }
    if sheet.0.is_none() {
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *start = None;
        *blocked = false;
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        *blocked = scrollbar.active()
            || thumbs.iter().any(|i| matches!(i, Interaction::Pressed | Interaction::Hovered));
    }
    if *blocked {
        return;
    }
    let Some(rcp) = image.iter().find(|r| r.cursor_over) else { return };
    let Some(n) = rcp.normalized else { return };

    let (cols, rows) = (grid.cols.max(1), grid.rows.max(1));
    let u = (n.x + 0.5).clamp(0.0, 0.999);
    let v = (n.y + 0.5).clamp(0.0, 0.999);
    let cur = UVec2::new((u * cols as f32).floor() as u32, (v * rows as f32).floor() as u32);

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if mouse.just_pressed(MouseButton::Left) {
        if ctrl {
            // Toggle a single cell, preserving click order.
            if let Some(i) = selection.cells.iter().position(|&c| c == cur) {
                selection.cells.remove(i);
            } else {
                selection.cells.push(cur);
            }
            *start = None;
            return;
        }
        // Plain drag anchors a rectangle that REPLACES the selection.
        *start = Some(cur);
    }
    let Some(anchor) = *start else { return };

    // Fill the rectangle [anchor, cur] row-major (top→bottom for a 1-wide
    // column, left→right for a 1-tall row).
    let (x0, x1) = (anchor.x.min(cur.x), anchor.x.max(cur.x));
    let (y0, y1) = (anchor.y.min(cur.y), anchor.y.max(cur.y));
    let mut cells = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            cells.push(UVec2::new(x, y));
        }
    }
    if selection.cells != cells {
        selection.cells = cells;
    }
}

/// Draw a border + 1-based order number on each selected cell (children of the
/// sheet image, percent-positioned so they track zoom). Rebuilds only when the
/// selection, grid, or image changes.
#[allow(clippy::too_many_arguments)]
fn update_cell_marks(
    selection: Res<SheetSelection>,
    grid: Res<SheetGrid>,
    fonts: Res<EmberFonts>,
    image_node: Query<(Entity, &ImageNode), With<SheetImageNode>>,
    marks: Query<Entity, With<SelCellMark>>,
    mut commands: Commands,
    mut built: Local<u64>,
) {
    let Ok((img, node)) = image_node.single() else {
        if !marks.is_empty() {
            for m in &marks {
                commands.entity(m).try_despawn();
            }
            *built = 0;
        }
        return;
    };
    let (cols, rows) = (grid.cols.max(1), grid.rows.max(1));

    let mut hasher = DefaultHasher::new();
    (img.to_bits(), node.image.id(), cols, rows).hash(&mut hasher);
    for c in &selection.cells {
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
    let color = accent_color();
    for (i, c) in selection.cells.iter().enumerate() {
        let mark = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(c.x as f32 / cols as f32 * 100.0),
                    top: Val::Percent(c.y as f32 / rows as f32 * 100.0),
                    width: Val::Percent(100.0 / cols as f32),
                    height: Val::Percent(100.0 / rows as f32),
                    border: UiRect::all(Val::Px(2.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(color.with_alpha(0.18)),
                BorderColor::all(color),
                SelCellMark,
                bevy::picking::Pickable::IGNORE,
                ChildOf(img),
            ))
            .id();
        let num = commands
            .spawn((
                Text::new(format!("{}", i + 1)),
                ui_font(&fonts.ui, 10.0),
                TextColor(Color::WHITE),
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        commands.entity(mark).add_child(num);
    }
}
