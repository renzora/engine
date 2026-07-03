//! Ghost preview of the tilemap brush in the 2D viewport.
//!
//! While the brush is armed (paint mode on, 2D edit view, cursor over the
//! viewport) the palette selection follows the cursor snapped to the active
//! layer's grid — one semi-transparent sprite per brush cell, so the user sees
//! exactly what a click will stamp and where. The preview entities carry no
//! `Name`, which keeps them out of the scene saver (it only serializes named
//! entities) and out of the hierarchy panel.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{EditorCamera2d, PlayModeState};
use renzora_tilemap::{TilemapLayer, TilesetHandle};

use crate::{cursor_to_world, ActiveTilemap, PaintRectDrag, TilemapBrush, TilemapPaintMode};

/// Ghost cells drawn for a pending Shift+drag rectangle are capped — a huge
/// region gets a translucent plain-colour overlay instead of thousands of
/// per-cell sprites.
const RECT_GHOST_MAX_CELLS: i64 = 2048;

/// Root of the ghost block; the per-cell sprites are its children.
#[derive(Component)]
pub(crate) struct BrushPreview;

/// Above sprites/tilemaps at their typical author z, but below the 2D
/// selection overlay (z 100), so the ghost never covers editor chrome.
const PREVIEW_Z: f32 = 90.0;
const PREVIEW_ALPHA: f32 = 0.55;

/// Everything the ghost needs from the active layer this frame.
struct Target {
    /// World position of the ghost root: the hovered cell's min corner — or
    /// the rectangle's min corner during a Shift+drag fill.
    origin: Vec2,
    image: Handle<Image>,
    tile_size: f32,
    atlas_px: f32,
    /// A pending Shift+drag rectangle, as `(cells_wide, cells_high, erasing)`.
    /// `None` = plain brush-block ghost under the cursor.
    rect: Option<(i32, i32, bool)>,
    /// Erase mode: the cursor ghost is a red cell, not the brush block.
    erase: bool,
}

/// Position (and when the brush/tileset/pending-rect changed, rebuild) the
/// ghost under the cursor; hide it whenever the brush isn't usable right now.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_brush_preview(
    mut commands: Commands,
    paint: Res<TilemapPaintMode>,
    brush: Res<TilemapBrush>,
    active: Res<ActiveTilemap>,
    rect_drag: Res<PaintRectDrag>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play: Option<Res<PlayModeState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
    layers: Query<(&TilemapLayer, &TilesetHandle, &GlobalTransform)>,
    mut roots: Query<(Entity, &mut Transform, &mut Visibility), With<BrushPreview>>,
    mut built: Local<Option<(Entity, u64)>>,
) {
    let target = 'find: {
        if !paint.active
            || play.is_some_and(|p| p.is_in_play_mode())
            || settings.map(|s| s.viewport_view).unwrap_or_default() != ViewportView::Two
        {
            break 'find None;
        }
        let Some(vs) = viewport else { break 'find None };
        // A rect drag keeps its ghost even when the cursor drifts off the
        // panel mid-drag (mirrors paint_tiles' commit-on-release rule).
        if !vs.hovered && rect_drag.0.is_none() {
            break 'find None;
        }
        let Some((layer, tileset, gt)) = active.0.and_then(|e| layers.get(e).ok()) else {
            break 'find None;
        };
        let ts = layer.tile_size;
        if ts <= 0.0 {
            break 'find None;
        }
        let layer_origin = gt.translation().truncate();

        if let Some((a, b, erase)) = rect_drag.0 {
            // Pending rectangle fill: ghost the whole region.
            let min = a.min(b);
            let max = a.max(b);
            break 'find Some(Target {
                origin: layer_origin + min.as_vec2() * ts,
                image: tileset.image.clone(),
                tile_size: ts,
                atlas_px: layer.atlas_tile_px.max(1) as f32,
                rect: Some((max.x - min.x + 1, max.y - min.y + 1, erase)),
                erase: paint.erase,
            });
        }

        let Ok(window) = windows.single() else { break 'find None };
        let Some(cursor) = window.cursor_position() else {
            break 'find None;
        };
        let Ok((camera, cam_gt)) = cameras_2d.single() else {
            break 'find None;
        };
        let Some(world) = cursor_to_world(cursor, &vs, camera, cam_gt) else {
            break 'find None;
        };
        // Same snap as paint_tiles, so the ghost sits exactly where a click
        // will stamp.
        let local = world - layer_origin;
        let cell = IVec2::new((local.x / ts).floor() as i32, (local.y / ts).floor() as i32);
        Some(Target {
            origin: layer_origin + cell.as_vec2() * ts,
            image: tileset.image.clone(),
            tile_size: ts,
            atlas_px: layer.atlas_tile_px.max(1) as f32,
            rect: None,
            erase: paint.erase,
        })
    };

    let Some(target) = target else {
        if let Ok((_, _, mut vis)) = roots.single_mut() {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    };

    let root = match roots.single_mut() {
        Ok((root, mut tf, mut vis)) => {
            tf.translation = target.origin.extend(PREVIEW_Z);
            if *vis != Visibility::Visible {
                *vis = Visibility::Visible;
            }
            root
        }
        Err(_) => commands
            .spawn((
                Transform::from_translation(target.origin.extend(PREVIEW_Z)),
                Visibility::Visible,
                BrushPreview,
            ))
            .id(),
    };

    // Rebuild the ghost sprites only when what they show changes — the brush
    // rect, the atlas, the cell metrics, or the pending fill region (or the
    // root itself was respawned). Moving the cursor only moves the root.
    let mut hasher = DefaultHasher::new();
    target.image.id().hash(&mut hasher);
    (brush.col, brush.row, brush.w, brush.h).hash(&mut hasher);
    target.tile_size.to_bits().hash(&mut hasher);
    target.atlas_px.to_bits().hash(&mut hasher);
    target.rect.hash(&mut hasher);
    target.erase.hash(&mut hasher);
    let key = hasher.finish();
    if *built == Some((root, key)) {
        return;
    }
    *built = Some((root, key));

    commands.entity(root).despawn_related::<Children>();
    let ts = target.tile_size;
    let px = target.atlas_px;

    if let Some((w, h, erase)) = target.rect {
        let count = w as i64 * h as i64;
        if erase || count > RECT_GHOST_MAX_CELLS {
            // Erase region (or an over-cap fill): one translucent overlay —
            // red for erase, white for a fill too large to ghost per cell.
            let size = Vec2::new(w as f32 * ts, h as f32 * ts);
            let color = if erase {
                Color::srgba(1.0, 0.35, 0.3, 0.25)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.18)
            };
            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(size),
                    ..default()
                },
                // Root is the region's min corner; sprites centre-anchor.
                Transform::from_xyz(size.x * 0.5, size.y * 0.5, 0.0),
                ChildOf(root),
            ));
            return;
        }
        // Ghost the fill: the brush block tiled across the region, pattern
        // anchored at the region's TOP-LEFT — exactly what release commits.
        let bw = brush.w.max(1) as i32;
        let bh = brush.h.max(1) as i32;
        for y in 0..h {
            for x in 0..w {
                let dx = x.rem_euclid(bw) as u32;
                let dy = ((h - 1 - y).rem_euclid(bh)) as u32;
                let u = (brush.col + dx) as f32 * px;
                let v = (brush.row + dy) as f32 * px;
                commands.spawn((
                    Sprite {
                        image: target.image.clone(),
                        rect: Some(Rect::new(u, v, u + px, v + px)),
                        custom_size: Some(Vec2::splat(ts)),
                        color: Color::srgba(1.0, 1.0, 1.0, PREVIEW_ALPHA),
                        ..default()
                    },
                    Transform::from_xyz(
                        x as f32 * ts + ts * 0.5,
                        y as f32 * ts + ts * 0.5,
                        0.0,
                    ),
                    ChildOf(root),
                ));
            }
        }
        return;
    }

    if target.erase {
        // Eraser cursor: erase strokes clear one cell per stroke cell (the
        // brush block never stamps), so ghost a single red cell.
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 0.35, 0.3, 0.25),
                custom_size: Some(Vec2::splat(ts)),
                ..default()
            },
            Transform::from_xyz(ts * 0.5, ts * 0.5, 0.0),
            ChildOf(root),
        ));
        return;
    }

    for (dx, dy, _) in brush.cells() {
        let u = (brush.col as i32 + dx) as f32 * px;
        let v = (brush.row as i32 + dy) as f32 * px;
        commands.spawn((
            Sprite {
                image: target.image.clone(),
                rect: Some(Rect::new(u, v, u + px, v + px)),
                custom_size: Some(Vec2::splat(ts)),
                color: Color::srgba(1.0, 1.0, 1.0, PREVIEW_ALPHA),
                ..default()
            },
            // Cell centers: the stamp grows right (+x) and down (−y), and the
            // hovered cell's min corner is the root — mirrors paint_tiles.
            Transform::from_xyz(
                dx as f32 * ts + ts * 0.5,
                -(dy as f32) * ts + ts * 0.5,
                0.0,
            ),
            ChildOf(root),
        ));
    }
}
