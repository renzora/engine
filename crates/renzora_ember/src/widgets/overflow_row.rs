//! A toolbar that **wraps** instead of hiding things, and can be rearranged by
//! dragging its groups around.
//!
//! Three earlier designs tried to solve "more controls than bar" by taking
//! controls *away* — into a dropdown menu, then a floating panel, then a tray
//! below the bar. Every one of them cost a measure-and-fold pass with its own
//! oscillation traps, and every one of them meant a control you wanted might not
//! be on screen. Wrapping to a second (or third) line has neither problem: the
//! toolbar simply gets taller, everything stays visible, and there is nothing to
//! compute — flexbox does it.
//!
//! Each group sits in a **holder** with a small grip on its left. A holder is one
//! flex item, so a group never splits across lines: one that doesn't fit moves
//! down whole. Hovering the grip highlights the group it belongs to; dragging it
//! carries the group under the cursor, with a blue [`RowGap`] marker opening at
//! the drop point so the neighbours visibly shift aside. Only the grip starts a
//! drag, so the controls themselves stay clickable at all times — no edit mode
//! to enter and leave.
//!
//! The order is published on the row as [`ArrangeOrder`] — a list of the group
//! keys, rewritten after every drop. A host that wants the arrangement to
//! survive a restart saves that list and writes it back; the row reorders itself
//! to match, and leaves anything it doesn't recognise where it already was.
//!
//! Two rules this depends on, both learned the hard way:
//!
//! - **Never orphan a live node to carry it.** A dragged holder is
//!   `position: absolute` but stays parented. Removing its `ChildOf` to float it
//!   makes it an untargeted layout root mid-frame and panics taffy from inside
//!   `ui_layout_system` — a hard crash, not a glitch.
//! - **Nothing in the chain may clip.** The dragged holder travels outside its
//!   container, and bevy_ui clips absolutely positioned descendants like
//!   everything else — the same trap that eats tooltips and submenu panels.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::font::{icon_text, EmberFonts};
use crate::theme::{accent, rgb, text_muted};

/// A group's stable name, for saving and restoring the toolbar's order.
#[derive(Component, Clone)]
pub struct ArrangeKey(pub String);

/// The row's current group order, newest-first-to-last. Written by the drag on
/// drop, and honoured when something else writes it — that's the hook for
/// persisting an arrangement.
#[derive(Component, Default, Clone, PartialEq)]
pub struct ArrangeOrder(pub Vec<String>);

/// One group's carrier: a grip plus the group itself.
#[derive(Component)]
pub(crate) struct RowHolder {
    /// The wrapping row this belongs to.
    row: Entity,
    /// The grip on its left.
    grip: Entity,
}

/// The grip on a holder — the only thing that starts a drag.
#[derive(Component)]
pub(crate) struct RowGrip(Entity);

/// The blue hole a drag opens at its drop point.
#[derive(Component)]
pub(crate) struct RowGap;

/// A wrapping toolbar row, and the identity every holder in it refers to.
#[derive(Component)]
pub struct ArrangeRow {
    gap: Entity,
}

/// The drag in flight.
#[derive(Resource, Default)]
pub(crate) struct RowDrag {
    holder: Option<Entity>,
    /// Cursor offset inside the holder at pick-up, so it doesn't jump.
    grab: Vec2,
    /// The holder's size — the size of the hole it leaves behind.
    size: Vec2,
}

/// Build a wrapping toolbar row. Mount the returned entity where the toolbar
/// goes — it fills the width it's given and grows taller as its contents need
/// more lines — then fill it with [`arrange_row_items`].
pub fn arrange_row(commands: &mut Commands, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                // Left to right, filling each line before starting the next.
                justify_content: JustifyContent::FlexStart,
                align_content: AlignContent::FlexStart,
                // The whole point: too many controls for one line become two.
                // A group never splits — the holders don't shrink, so one that
                // doesn't fit moves down whole rather than being squeezed.
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(2.0),
                row_gap: Val::Px(2.0),
                // NOT clipped, and no ancestor of it may be: a holder being
                // dragged is absolutely positioned and travels outside this box.
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            ArrangeOrder::default(),
            Name::new(format!("{name}-arrange-row")),
        ))
        .id();
    let gap = commands
        .spawn((
            Node {
                display: Display::None,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.55)),
            Pickable::IGNORE,
            RowGap,
            Name::new(format!("{name}-drop-gap")),
        ))
        .id();
    commands.entity(row).insert(ArrangeRow { gap });
    commands.entity(gap).insert(ChildOf(row));
    row
}

/// Fill a row built by [`arrange_row`]. Each entry is a group and the stable key
/// it's saved under. Returns one holder per group, in the order given — bind
/// visibility on those rather than on the groups themselves, so a hidden group
/// doesn't leave its grip behind.
pub fn arrange_row_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    row: Entity,
    kids: &[(Entity, &str)],
) -> Vec<Entity> {
    let holders: Vec<Entity> = kids
        .iter()
        .map(|(kid, key)| {
            let holder = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(1.0),
                        padding: UiRect::right(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Name::new("toolbar-group"),
                ))
                .id();
            let grip = commands
                .spawn((
                    Node {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::horizontal(Val::Px(1.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                    crate::widgets::HoverTooltip::new("Drag to move this group"),
                    RowGrip(holder),
                    Name::new("toolbar-group-grip"),
                ))
                .id();
            let dots = icon_text(commands, &fonts.phosphor, "dots-six-vertical", text_muted(), 11.0);
            commands.entity(dots).insert(bevy::ui::FocusPolicy::Pass);
            commands.entity(grip).add_child(dots);
            commands
                .entity(holder)
                .insert((RowHolder { row, grip }, ArrangeKey(key.to_string())));
            commands.entity(holder).add_children(&[grip, *kid]);
            holder
        })
        .collect();
    commands.entity(row).add_children(&holders);
    holders
}

/// Light up the group a grip belongs to while it's hovered or being dragged, so
/// it's obvious what the grip will pick up.
pub(crate) fn arrange_highlight(
    drag: Res<RowDrag>,
    holders: Query<(Entity, &RowHolder)>,
    grips: Query<&Interaction>,
    mut backgrounds: Query<&mut BackgroundColor>,
) {
    for (holder, h) in &holders {
        let hot = drag.holder == Some(holder)
            || matches!(
                grips.get(h.grip),
                Ok(Interaction::Hovered) | Ok(Interaction::Pressed)
            );
        let Ok(mut bg) = backgrounds.get_mut(holder) else {
            continue;
        };
        let want = if hot {
            rgb(accent()).with_alpha(0.20)
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Drag a group by its grip to move it along the toolbar.
///
/// While the button is held the holder is absolutely positioned and follows the
/// cursor, and the row's blue [`RowGap`] sits where it would land — so the groups
/// around it shift aside as you go. On release the row's children are rewritten
/// with the holder at that position.
pub(crate) fn arrange_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drag: ResMut<RowDrag>,
    rows: Query<&ArrangeRow>,
    grips: Query<(&Interaction, &RowGrip)>,
    holders: Query<&RowHolder>,
    keys: Query<&ArrangeKey>,
    geom: Query<(&UiGlobalTransform, &ComputedNode)>,
    children: Query<&Children>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
) {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.holder.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        let Some((_, grip)) = grips.iter().find(|(i, _)| **i == Interaction::Pressed) else {
            return;
        };
        let holder = grip.0;
        let (Some(cursor), Some((top_left, size))) = (cursor, node_rect(holder, &geom)) else {
            return;
        };
        // Out of the flow — the line closes up behind it — but still parented.
        commands
            .entity(holder)
            .insert((GlobalZIndex(9500), Pickable::IGNORE));
        if let Ok(mut node) = nodes.get_mut(holder) {
            node.position_type = PositionType::Absolute;
        }
        drag.holder = Some(holder);
        drag.grab = cursor - top_left;
        drag.size = size;
        return;
    }

    let Some(holder) = drag.holder else { return };
    let Ok(h) = holders.get(holder) else {
        drag.holder = None;
        return;
    };
    let Ok(row) = rows.get(h.row) else {
        drag.holder = None;
        return;
    };
    let gap = row.gap;

    // Where it would land: how many groups the cursor has passed, in reading
    // order. Measured against each group's own box — a fraction of the row's
    // width says nothing about where the groups actually are once the row wraps.
    let sibs: Vec<Entity> = children
        .get(h.row)
        .map(|c| c.iter().filter(|e| *e != holder && *e != gap).collect())
        .unwrap_or_default();
    let at = cursor.map(|cursor| {
        sibs.iter()
            .filter(|e| {
                node_rect(**e, &geom)
                    .map(|(top_left, size)| {
                        if cursor.y > top_left.y + size.y {
                            true // a line above the cursor's
                        } else if cursor.y < top_left.y {
                            false // a line below it
                        } else {
                            top_left.x + size.x * 0.5 < cursor.x // same line
                        }
                    })
                    .unwrap_or(false)
            })
            .count()
    });

    if mouse.pressed(MouseButton::Left) {
        // Carry it. `left`/`top` on an absolutely positioned child are relative
        // to its parent, so the parent's own top-left comes off the cursor.
        let origin = node_rect(h.row, &geom).map(|(tl, _)| tl).unwrap_or(Vec2::ZERO);
        if let (Some(cursor), Ok(mut node)) = (cursor, nodes.get_mut(holder)) {
            let (left, top) = (
                Val::Px(cursor.x - origin.x - drag.grab.x),
                Val::Px(cursor.y - origin.y - drag.grab.y),
            );
            if node.left != left || node.top != top {
                node.left = left;
                node.top = top;
            }
        }
        if let Some(at) = at {
            if let Ok(mut node) = nodes.get_mut(gap) {
                node.display = Display::Flex;
                node.width = Val::Px(drag.size.x.max(24.0));
                node.height = Val::Px(drag.size.y.max(20.0));
            }
            let mut kids = sibs.clone();
            kids.insert(at.min(kids.len()), gap);
            // The dragged holder keeps its place in the child list: dropping it
            // would orphan a live node, which is what panics taffy. It's
            // absolute, so it takes no space wherever it sits in the order.
            kids.push(holder);
            let now: Vec<Entity> = children
                .get(h.row)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            if now != kids {
                commands.entity(h.row).replace_children(&kids);
            }
        }
        return;
    }

    // Dropped: back into the flow, marker away.
    drag.holder = None;
    commands
        .entity(holder)
        .remove::<GlobalZIndex>()
        .remove::<Pickable>();
    if let Ok(mut node) = nodes.get_mut(holder) {
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;
    }
    if let Ok(mut node) = nodes.get_mut(gap) {
        node.display = Display::None;
    }
    let Some(at) = at else { return };
    let mut kids = sibs;
    kids.insert(at.min(kids.len()), holder);
    // The marker lives on the row between drags, so it has to survive the
    // rewrite — anything left out would be orphaned.
    kids.push(gap);
    let order: Vec<String> = kids
        .iter()
        .filter_map(|e| keys.get(*e).ok().map(|k| k.0.clone()))
        .collect();
    commands.entity(h.row).replace_children(&kids);
    // Published, not just applied: this is what a host saves.
    commands.entity(h.row).insert(ArrangeOrder(order));
}

/// Reorder a row's groups when its [`ArrangeOrder`] is written from outside —
/// a restored arrangement, typically.
///
/// Skipped mid-drag, and skipped when the row already matches, so the drag's own
/// write doesn't bounce back through here. Keys the row doesn't have are ignored
/// and holders the order doesn't mention keep their relative places at the end,
/// so a saved arrangement from a build with different groups still applies as
/// far as it makes sense.
pub(crate) fn arrange_apply_order(
    drag: Res<RowDrag>,
    rows: Query<(Entity, &ArrangeOrder, &Children), Changed<ArrangeOrder>>,
    keys: Query<&ArrangeKey>,
    gaps: Query<(), With<RowGap>>,
    mut commands: Commands,
) {
    if drag.holder.is_some() {
        return;
    }
    for (row, order, children) in &rows {
        let current: Vec<Entity> = children.iter().collect();
        let mut holders: Vec<Entity> = current
            .iter()
            .copied()
            .filter(|e| !gaps.contains(*e))
            .collect();
        let mut sorted: Vec<Entity> = Vec::with_capacity(holders.len());
        for key in &order.0 {
            if let Some(pos) = holders
                .iter()
                .position(|e| keys.get(*e).map(|k| &k.0 == key).unwrap_or(false))
            {
                sorted.push(holders.remove(pos));
            }
        }
        sorted.extend(holders);
        // The markers live on the row between drags and have to survive the
        // rewrite; anything left out of `replace_children` would be orphaned.
        sorted.extend(current.iter().copied().filter(|e| gaps.contains(*e)));
        if sorted != current {
            commands.entity(row).replace_children(&sorted);
        }
    }
}

/// A node's top-left corner and size in logical window pixels.
///
/// `UiGlobalTransform`, not `GlobalTransform`: bevy 0.19's layout writes the
/// node's placement there (its *centre*, in physical px), and a UI node's
/// `GlobalTransform` is left at the origin.
fn node_rect(e: Entity, geom: &Query<(&UiGlobalTransform, &ComputedNode)>) -> Option<(Vec2, Vec2)> {
    let (ugt, cn) = geom.get(e).ok()?;
    let inv = cn.inverse_scale_factor();
    let size = cn.size() * inv;
    Some((ugt.translation * inv - size * 0.5, size))
}
