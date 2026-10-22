//! Rubber-band (drag) selection for the hierarchy tree.
//!
//! Press on empty space in the tree viewport (below the last row, where there's
//! no row to click or drag-reorder) and drag: every row the band sweeps over
//! becomes selected. Ctrl/Shift keep the prior selection so a sweep *adds* to it;
//! a plain sweep replaces it. A plain *click* on empty space (or a band that
//! never touches a row) leaves the selection alone: it used to clear it on press,
//! which made empty hierarchy space the one place in the editor where clicking
//! away deselected, and the selection should only change when a row is picked.
//! Escape / the explicit deselect paths are untouched. The band autoscrolls the list at its top/bottom edges so it can reach off-screen
//! rows.
//!
//! Starting only on empty space is what keeps this from fighting the row
//! reorder-drag (`drag.rs`), which arms on a press *over a row* — the two never
//! see the same press.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, ScrollPosition, UiGlobalTransform};

use renzora_editor_framework::EditorSelection;
use renzora_ember::resize::ResizeBusy;
use renzora_ember::theme::{accent, rgb};
use renzora_ember::widgets::{EmberScroll, ScrollbarBusy};

use super::components::HierRowClick;
use super::drag::HierDrag;
use super::rename::HierRename;
use super::HierScrollContent;

/// Live rubber-band state, in window (logical) px.
#[derive(Resource, Default)]
pub(crate) struct HierMarquee {
    /// Press origin; `Some` while a band is being dragged.
    start: Option<Vec2>,
    /// Latest cursor position (the band's other corner).
    current: Option<Vec2>,
    /// Selection captured when the band began — swept rows add to this, so a
    /// Ctrl/Shift-started band extends the prior selection instead of replacing it.
    pre: Vec<Entity>,
    /// Selection at press time, restored whenever the band
    /// sweeps no rows. A plain press no longer clears the selection, so a band
    /// that grows over rows and then shrinks back off them must put back what was
    /// there rather than leave its last sweep behind.
    orig: Vec<Entity>,
    /// Whether this band has written a sweep into the selection, so an empty
    /// sweep only restores `orig` once instead of rewriting it every frame.
    touched: bool,
}

impl HierMarquee {
    /// Whether a band is currently active (used to pause selection-reveal so the
    /// sweep isn't yanked around).
    pub(crate) fn active(&self) -> bool {
        self.start.is_some()
    }
}

/// The rubber-band rectangle overlay (top-level, unclipped by the panel).
#[derive(Component)]
pub(crate) struct HierMarqueeRect;

/// Vertical screen band `[top, bottom]` (logical px) a row occupies, from its
/// UI transform + computed size. Rows are full-width, so only the Y span matters
/// for a list marquee.
fn row_band(cn: &ComputedNode, ugt: &UiGlobalTransform) -> (f32, f32) {
    let inv = cn.inverse_scale_factor();
    let half = cn.size().y * inv * 0.5;
    let cy = ugt.translation.y * inv;
    (cy - half, cy + half)
}

/// Drive the band: begin on an empty press over the tree, extend the selection
/// while held, and clear on release.
pub(crate) fn hier_marquee(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    selection: Option<Res<EditorSelection>>,
    rename: Res<HierRename>,
    drag: Res<HierDrag>,
    rows: Query<(&HierRowClick, &ComputedNode, &UiGlobalTransform)>,
    content: Query<Entity, With<HierScrollContent>>,
    parents: Query<&ChildOf>,
    viewports: Query<&RelativeCursorPosition>,
    scrollbar: Res<ScrollbarBusy>,
    resizing: Res<ResizeBusy>,
    mut marquee: ResMut<HierMarquee>,
) {
    if mouse.just_released(MouseButton::Left) {
        marquee.start = None;
        marquee.current = None;
        marquee.pre.clear();
        marquee.orig.clear();
        marquee.touched = false;
        return;
    }
    let Some(selection) = selection else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };

    // Begin on a press that landed in empty tree space: over the scroll viewport,
    // not over any row, and never while a row reorder-drag or inline rename is in
    // flight. Suppressing it over rows is what keeps reorder-drag conflict-free.
    //
    // The two busy flags cover the widgets that sit *over* the tree's rect but
    // aren't part of it, so `cursor_over` alone can't tell them apart from empty
    // list space: the scrollbar inside the panel, and the resize handles that
    // overhang its edges (GH #81 — dragging the panel's resize grip used to
    // sweep-select whatever rows the drag passed).
    if mouse.just_pressed(MouseButton::Left)
        && marquee.start.is_none()
        && rename.0.is_none()
        && !drag.active
        && !scrollbar.active()
        && !resizing.active()
    {
        let over_tree = content
            .iter()
            .next()
            .and_then(|list| parents.get(list).ok())
            .and_then(|c| viewports.get(c.parent()).ok())
            .is_some_and(|rcp| rcp.cursor_over);
        let on_row = rows.iter().any(|(_, cn, ugt)| {
            let (top, bottom) = row_band(cn, ugt);
            cursor.y >= top && cursor.y <= bottom
        });
        if over_tree && !on_row {
            let additive = keyboard.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::ShiftLeft,
                KeyCode::ShiftRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ]);
            marquee.start = Some(cursor);
            marquee.current = Some(cursor);
            marquee.touched = false;
            // Not cleared here: a plain click on empty space keeps the selection,
            // and only a band that actually sweeps rows replaces it.
            marquee.orig = selection.get_all();
            if additive {
                marquee.pre = marquee.orig.clone();
            } else {
                marquee.pre.clear();
            }
        }
    }

    // Extend the sweep while held.
    if mouse.pressed(MouseButton::Left) {
        if let Some(start) = marquee.start {
            marquee.current = Some(cursor);
            let (min_y, max_y) = (start.y.min(cursor.y), start.y.max(cursor.y));
            // Swept rows, ordered top→bottom so the first stays the primary.
            let mut swept: Vec<(f32, Entity)> = rows
                .iter()
                .filter_map(|(row, cn, ugt)| {
                    let (top, bottom) = row_band(cn, ugt);
                    (top <= max_y && bottom >= min_y).then_some((top, row.entity))
                })
                .collect();
            swept.sort_by(|a, b| a.0.total_cmp(&b.0));

            if swept.is_empty() {
                // Nothing under the band: put back the press-time selection (if
                // a sweep replaced it) instead of writing an empty one.
                if marquee.touched {
                    selection.set_multiple(marquee.orig.clone());
                    marquee.touched = false;
                }
                return;
            }
            marquee.touched = true;

            let mut sel = marquee.pre.clone();
            for (_, e) in swept {
                if !sel.contains(&e) {
                    sel.push(e);
                }
            }
            selection.set_multiple(sel);
        }
    }
}

/// Draw/update the band rectangle as a top-level overlay (unclipped, like the
/// asset browser's), and despawn it when the band ends.
pub(crate) fn hier_marquee_overlay(
    mut commands: Commands,
    marquee: Res<HierMarquee>,
    mut rects: Query<(Entity, &mut Node), With<HierMarqueeRect>>,
) {
    if let (Some(a), Some(b)) = (marquee.start, marquee.current) {
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
                HierMarqueeRect,
                Name::new("hier-marquee"),
            ));
        }
    } else {
        for (e, _) in &rects {
            commands.entity(e).despawn();
        }
    }
}

/// While a band is dragged, scroll the list when the cursor nears the viewport's
/// top/bottom edge — so a rubber-band can reach off-screen rows. Speed ramps with
/// how deep into the edge band the cursor sits.
pub(crate) fn hier_marquee_autoscroll(
    marquee: Res<HierMarquee>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    content: Query<Entity, With<HierScrollContent>>,
    parents: Query<&ChildOf>,
    mut viewports: Query<(&mut EmberScroll, &ComputedNode, &UiGlobalTransform), With<ScrollPosition>>,
) {
    const EDGE: f32 = 34.0;
    const MAX_SPEED: f32 = 16.0;

    if marquee.start.is_none() || !mouse.pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    // The tree's scroll viewport is the marked content node's parent.
    let Some(vp) = content
        .iter()
        .next()
        .and_then(|list| parents.get(list).ok())
        .map(|c| c.parent())
    else {
        return;
    };
    let Ok((mut s, cn, ugt)) = viewports.get_mut(vp) else {
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
