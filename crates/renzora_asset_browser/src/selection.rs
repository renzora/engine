//! Multi-selection: the rubber-band marquee (with edge autoscroll), the display
//! order shift-range select reads, and Ctrl+A.

use std::path::PathBuf;

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_ember::theme::{accent, rgb};
use renzora_ember::widgets::{EmberScroll, ScrollbarBusy};

use crate::state::{AssetTile, GridArea, MarqueeRect, NativeAssets};

/// Keep `visible_order` in sync with the grid's current folder + sort so
/// shift-range selection knows the display order.
pub(crate) fn track_visible_order(mut state: ResMut<NativeAssets>) {
    // Reads the shared cache (refreshed by `refresh_listing`) — no filesystem.
    let order: Vec<PathBuf> = state.listing.iter().map(|e| e.path.clone()).collect();
    if state.visible_order != order {
        state.visible_order = order;
    }
}

/// Rubber-band selection: pressing in empty grid space and dragging selects
/// every tile the rectangle touches. Ctrl/Shift keep the prior selection
/// (sweeping adds to it). Mirrors the egui browser's marquee.
pub(crate) fn marquee_select(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    grid: Query<&bevy::ui::RelativeCursorPosition, With<GridArea>>,
    tiles: Query<(&AssetTile, &Interaction, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    scrollbar: Res<ScrollbarBusy>,
    resizing: Res<renzora_ember::resize::ResizeBusy>,
    mut state: ResMut<NativeAssets>,
) {
    if mouse.just_released(MouseButton::Left) {
        state.marquee_start = None;
        state.marquee_current = None;
        state.pre_marquee.clear();
        return;
    }
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };

    // Begin on a press over the grid that didn't land on a tile. Suppressed while
    // an inline rename is active so clicking into its field doesn't start a sweep,
    // and while the press is on the scrollbar (grabbing it to scroll must not
    // start a marquee in the content beneath) or on a resize handle, whose grab
    // strip overhangs the panel edge and so lands inside the grid's rect (same
    // bug the hierarchy's marquee had — GH #81).
    if mouse.just_pressed(MouseButton::Left)
        && state.marquee_start.is_none()
        && state.renaming.is_none()
        && !scrollbar.active()
        && !resizing.active()
    {
        let over_grid = grid.iter().any(|r| r.cursor_over);
        let on_tile = tiles.iter().any(|(_, i, _, _)| *i == Interaction::Pressed);
        if over_grid && !on_tile {
            let keep = keyboard.pressed(KeyCode::ControlLeft)
                || keyboard.pressed(KeyCode::ControlRight)
                || keyboard.pressed(KeyCode::ShiftLeft)
                || keyboard.pressed(KeyCode::ShiftRight);
            state.marquee_start = Some(cursor);
            state.marquee_current = Some(cursor);
            if keep {
                state.pre_marquee = state.selection.clone();
            } else {
                state.selection.clear();
                state.pre_marquee.clear();
            }
        }
    }

    // Update the sweep while held.
    if mouse.pressed(MouseButton::Left) {
        if let Some(start) = state.marquee_start {
            state.marquee_current = Some(cursor);
            let (min, max) = (start.min(cursor), start.max(cursor));
            let mut sel = state.pre_marquee.clone();
            for (tile, _, cn, ugt) in &tiles {
                let scale = cn.inverse_scale_factor();
                let half = cn.size() * scale * 0.5;
                let center = ugt.translation * scale;
                let (tmin, tmax) = (center - half, center + half);
                // AABB overlap test against the marquee rect.
                if tmin.x <= max.x && tmax.x >= min.x && tmin.y <= max.y && tmax.y >= min.y {
                    sel.insert(tile.path.clone());
                }
            }
            state.selection = sel;
            state.selected = state.selection.iter().next().cloned();
        }
    }
}

/// Draws/updates the marquee rectangle as a top-level overlay (unclipped by the
/// panel), and despawns it when the marquee ends.
pub(crate) fn marquee_overlay(
    mut commands: Commands,
    state: Res<NativeAssets>,
    mut rects: Query<(Entity, &mut Node), With<MarqueeRect>>,
) {
    if let (Some(a), Some(b)) = (state.marquee_start, state.marquee_current) {
        let min = a.min(b);
        let size = (a.max(b) - min).max(Vec2::ZERO);
        if let Some((_, mut n)) = rects.iter_mut().next() {
            n.left = Val::Px(min.x);
            n.top = Val::Px(min.y);
            n.width = Val::Px(size.x);
            n.height = Val::Px(size.y);
        } else {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(min.x),
                    top: Val::Px(min.y),
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(accent()).with_alpha(0.15)),
                BorderColor::all(rgb(accent())),
                GlobalZIndex(9_000),
                Pickable::IGNORE,
                MarqueeRect,
                Name::new("asset-marquee"),
            ));
        }
    } else {
        for (e, _) in &rects {
            commands.entity(e).despawn();
        }
    }
}

/// While a marquee is being dragged, scroll the grid when the cursor nears the
/// viewport's top/bottom edge — so a rubber-band can reach off-screen tiles.
/// Speed ramps with how deep into the edge band the cursor is.
pub(crate) fn marquee_autoscroll(
    state: Res<NativeAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    grid_area: Query<&Children, With<GridArea>>,
    mut scrolls: Query<(&mut EmberScroll, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
) {
    // How close (px) to an edge triggers scrolling, and the max px/frame at the
    // very edge.
    const EDGE: f32 = 34.0;
    const MAX_SPEED: f32 = 16.0;

    if state.marquee_start.is_none() || !mouse.pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    // The grid's `scroll_view` outer (GridArea) wraps the `EmberScroll` viewport
    // as its first child.
    let Ok(kids) = grid_area.single() else {
        return;
    };
    let Some(viewport) = kids.iter().find(|&e| scrolls.contains(e)) else {
        return;
    };
    let Ok((mut s, cn, ugt)) = scrolls.get_mut(viewport) else {
        return;
    };
    let inv = cn.inverse_scale_factor();
    let half_h = cn.size().y * inv * 0.5;
    let center_y = ugt.translation.y * inv;
    let (top, bottom) = (center_y - half_h, center_y + half_h);
    if cursor.y < top + EDGE {
        let t = ((top + EDGE - cursor.y) / EDGE).clamp(0.0, 1.0);
        s.nudge(-t * MAX_SPEED);
    } else if cursor.y > bottom - EDGE {
        let t = ((cursor.y - (bottom - EDGE)) / EDGE).clamp(0.0, 1.0);
        s.nudge(t * MAX_SPEED);
    }
}

/// Ctrl/Cmd+A selects every visible entry in the current folder. Gated on the
/// grid being hovered so it doesn't hijack select-all from the viewport,
/// hierarchy, or a focused text field elsewhere in the editor.
///
/// It also publishes [`SelectAllClaimed`](renzora::core::SelectAllClaimed) every
/// frame from the grid-hover state, so the hierarchy's "select all entities"
/// stands down while the cursor is over the file grid — otherwise both fire on
/// the same `Ctrl+A`.
pub(crate) fn select_all_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    grid: Query<&bevy::ui::RelativeCursorPosition, With<GridArea>>,
    mut state: ResMut<NativeAssets>,
    mut claimed: ResMut<renzora::core::SelectAllClaimed>,
) {
    let over_grid = grid.iter().any(|r| r.cursor_over) && state.renaming.is_none();
    claimed.0 = over_grid;

    let ctrl = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    if !ctrl || !keys.just_pressed(KeyCode::KeyA) || !over_grid {
        return;
    }
    if state.visible_order.is_empty() {
        return;
    }
    state.selection = state.visible_order.iter().cloned().collect();
    state.selected = state.visible_order.first().cloned();
    state.selection_anchor = state.visible_order.first().cloned();
}
