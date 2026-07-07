//! Rubber-band (drag) selection for the hierarchy tree.
//!
//! Press on empty space in the tree viewport (below the last row, where there's
//! no row to click or drag-reorder) and drag: every row the band sweeps over
//! becomes selected. Ctrl/Shift keep the prior selection so a sweep *adds* to it;
//! a plain press first clears it (so clicking empty space also deselects). The
//! band autoscrolls the list at its top/bottom edges so it can reach off-screen
//! rows.
//!
//! Starting only on empty space is what keeps this from fighting the row
//! reorder-drag (`drag.rs`), which arms on a press *over a row* — the two never
//! see the same press.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, ScrollPosition, UiGlobalTransform};

use renzora_editor_framework::EditorSelection;
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
    mut marquee: ResMut<HierMarquee>,
) {
    if mouse.just_released(MouseButton::Left) {
        marquee.start = None;
        marquee.current = None;
        marquee.pre.clear();
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
    if mouse.just_pressed(MouseButton::Left)
        && marquee.start.is_none()
        && rename.0.is_none()
        && !drag.active
        && !scrollbar.active()
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
            if additive {
                marquee.pre = selection.get_all();
            } else {
                marquee.pre.clear();
                // A plain empty-space press deselects; a sweep re-fills below.
                selection.clear();
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
