//! Drag a component section by the grip in its header to move it up or down the
//! inspector.
//!
//! **Nothing moves while the drag is in flight.** A thin accent line marks where
//! the section would land, and the sections themselves stay exactly where they
//! are until the button comes up. That is not restraint for its own sake: a list
//! that reorders live feeds its own input back into itself, because the sibling
//! whose midpoint decides the drop index shifts the moment the drop happens, and
//! sections here differ in height by a factor of twenty (a collapsed header
//! against an open Material drawer). An indicator has no feedback loop at all.
//!
//! The drop is applied by *recording the new order* ([`super::order`]) and
//! letting the panel rebuild, rather than by re-parenting the section entities.
//! The ranking is what has to be right — it outlives this entity and this
//! session — so making it the only thing a drop writes means the screen cannot
//! disagree with what was saved.
//!
//! Only the grip starts a drag, and it blocks the press from reaching the header
//! behind it: `section_toggle` fires on `Pressed`, not on release, so a header
//! that started drags would collapse the section the instant you grabbed it.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, ScrollPosition, UiGlobalTransform};

use renzora_ember::font::EmberFonts;
use renzora_ember::theme::{accent, rgb, text_muted};
use renzora_ember::widgets::{EmberScroll, HoverTooltip};

use super::order::InspectorSectionOrder;
use super::InspectorRoot;

/// How far the cursor must travel off a pressed grip before it counts as a drag.
/// Matches the hierarchy's, so both panels feel the same.
const DRAG_THRESHOLD: f32 = 5.0;

/// Thickness of the drop line, in logical pixels.
const LINE_H: f32 = 2.0;

/// On a section root: which component it is, so a drop can name it in the saved
/// ranking. Also what marks a child of [`InspectorRoot`] as a section at all —
/// the drop line is a child too, and must not be counted as one.
#[derive(Component)]
pub(crate) struct InspectorSectionRoot {
    pub(crate) type_id: &'static str,
    /// The section's header, which the drag tints to show what it is carrying.
    /// Held here rather than walked to through `Children` because the tint has
    /// to land on the header itself: a collapsed section *is* its header, so a
    /// tint on the root would be covered by it and show nothing at all.
    pub(crate) header: Entity,
}

/// The grip in a section header; points back at the section it moves.
#[derive(Component)]
pub(crate) struct SectionGrip {
    root: Entity,
}

/// The accent line showing where the dragged section would land.
#[derive(Component)]
pub(crate) struct SectionDropLine;

/// The drag in flight.
#[derive(Resource, Default)]
pub(crate) struct SectionDrag {
    /// A press on a grip that has not yet moved far enough to be a drag.
    press: Option<(Entity, Vec2)>,
    /// The section root being dragged.
    active: Option<Entity>,
    /// Where it would land: an index into the *other* sections, top to bottom.
    at: usize,
}

impl SectionDrag {
    /// The section root being carried, if any. Read by
    /// `systems::stripe_collapsed_headers`, which would otherwise repaint the
    /// dragged header back to its stripe on the very next frame.
    pub(crate) fn dragged(&self) -> Option<Entity> {
        self.active
    }
}

/// Build the drag grip for a section header.
pub(super) fn build_grip(commands: &mut Commands, fonts: &EmberFonts, root: Entity) -> Entity {
    let grip = super::phosphor_glyph(commands, fonts, "dots-six-vertical", text_muted(), 13.0);
    commands.entity(grip).insert((
        Interaction::default(),
        // Overrides the `FocusPolicy::Pass` every inspector glyph gets, for the
        // same reason the enable toggle and the trash are `Block`: the press
        // must not reach the header, which would toggle the section.
        FocusPolicy::Block,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
        HoverTooltip::new(renzora::lang::t("inspector.component.reorder")),
        SectionGrip { root },
    ));
    grip
}

/// Press a grip, move, release: the whole gesture, plus the drop line.
pub(super) fn section_reorder_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    grips: Query<(&Interaction, &SectionGrip)>,
    roots: Query<&InspectorSectionRoot>,
    container_q: Query<(Entity, Option<&Children>), With<InspectorRoot>>,
    geom: Query<(&UiGlobalTransform, &ComputedNode)>,
    lines: Query<Entity, With<SectionDropLine>>,
    mut nodes: Query<&mut Node>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut drag: ResMut<SectionDrag>,
    mut order: ResMut<InspectorSectionOrder>,
    mut commands: Commands,
) {
    let cursor = windows.iter().find_map(|w| w.cursor_position());
    let Ok((container, children)) = container_q.single() else {
        return;
    };
    let sections: Vec<Entity> = children
        .map(|c| c.iter().filter(|e| roots.contains(*e)).collect())
        .unwrap_or_default();

    // Button up ends the gesture, whether or not we saw the release itself (a
    // release outside the window never reports `just_released`).
    if !mouse.pressed(MouseButton::Left) {
        if let Some(dragged) = drag.active.take() {
            record_drop(&mut order, &roots, &sections, dragged, drag.at);
            // The tint is not undone here: `stripe_collapsed_headers` owns every
            // header's colour and repaints this one the moment it stops being
            // skipped, which is now.
        }
        drag.press = None;
        show_line(&lines, &mut nodes, None);
        return;
    }

    if drag.active.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            if let (Some(c), Some((_, grip))) = (
                cursor,
                grips.iter().find(|(i, _)| **i == Interaction::Pressed),
            ) {
                drag.press = Some((grip.root, c));
            }
        }
        // A press that never moves is a click on the grip, and a click on the
        // grip does nothing — so the drag only starts past the threshold.
        match (drag.press, cursor) {
            (Some((root, origin)), Some(c)) if c.distance(origin) > DRAG_THRESHOLD => {
                drag.active = Some(root);
                // Painted once, not every frame: from here until the drop,
                // `stripe_collapsed_headers` leaves this header alone.
                if let Some(header) = roots.get(root).ok().map(|r| r.header) {
                    if let Ok(mut bg) = backgrounds.get_mut(header) {
                        bg.0 = rgb(accent()).with_alpha(0.45);
                    }
                }
            }
            _ => return,
        }
    }

    let Some(dragged) = drag.active else {
        return;
    };
    // The panel rebuilt under the drag (the selection changed, or a component
    // was removed): the section being carried no longer exists.
    if !sections.contains(&dragged) {
        drag.active = None;
        drag.press = None;
        show_line(&lines, &mut nodes, None);
        return;
    }
    // No cursor (it left the window): hold the last target rather than snapping
    // the line to the top.
    let Some(cursor) = cursor else {
        return;
    };

    let others: Vec<Entity> = sections.iter().copied().filter(|e| *e != dragged).collect();
    // Where it would land: how many of the other sections the cursor has passed,
    // measured against each one's own midpoint. A fraction of the list's height
    // would be wrong the moment two sections differ in size, which they always
    // do — a collapsed header is 22px and an open drawer several hundred.
    drag.at = others
        .iter()
        .filter(|e| {
            node_rect(**e, &geom)
                .map(|(top_left, size)| top_left.y + size.y * 0.5 < cursor.y)
                .unwrap_or(false)
        })
        .count();

    // The seam the line sits on: the top of the section that would follow it, or
    // the bottom of the one it would follow.
    let Some((container_tl, _)) = node_rect(container, &geom) else {
        return;
    };
    let seam = if drag.at == 0 {
        others
            .first()
            .and_then(|e| node_rect(*e, &geom))
            .map(|(tl, _)| tl.y)
    } else {
        others
            .get(drag.at - 1)
            .and_then(|e| node_rect(*e, &geom))
            .map(|(tl, size)| tl.y + size.y)
    };
    let Some(seam) = seam else {
        return;
    };
    // Absolute insets are measured from the container's own box, and both rects
    // are in window space, so the difference is the offset inside it — scrolled
    // or not.
    let top = Val::Px(seam - container_tl.y - LINE_H * 0.5);
    if lines.iter().next().is_none() {
        spawn_line(&mut commands, container, top);
    } else {
        show_line(&lines, &mut nodes, Some(top));
    }
}

/// While a section is dragged, scroll the panel when the cursor nears the
/// viewport's top/bottom edge, so a section can be moved past what fits on
/// screen. Speed ramps with how deep into the edge band the cursor sits (the
/// same shape as the hierarchy's marquee autoscroll).
pub(super) fn section_reorder_autoscroll(
    drag: Res<SectionDrag>,
    windows: Query<&Window>,
    root: Query<Entity, With<InspectorRoot>>,
    parents: Query<&ChildOf>,
    mut viewports: Query<(&mut EmberScroll, &ComputedNode, &UiGlobalTransform), With<ScrollPosition>>,
) {
    const EDGE: f32 = 34.0;
    const MAX_SPEED: f32 = 16.0;

    if drag.active.is_none() {
        return;
    }
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let Ok(root) = root.single() else {
        return;
    };
    let Ok(vp) = parents.get(root).map(|c| c.parent()) else {
        return;
    };
    let Ok((mut scroll, node, xf)) = viewports.get_mut(vp) else {
        return;
    };
    let inv = node.inverse_scale_factor();
    let half_h = node.size().y * inv * 0.5;
    let center_y = xf.translation.y * inv;
    let (top, bottom) = (center_y - half_h, center_y + half_h);
    if cursor.y < top + EDGE {
        let t = ((top + EDGE - cursor.y) / EDGE).clamp(0.0, 1.0);
        scroll.nudge(-t * MAX_SPEED);
    } else if cursor.y > bottom - EDGE {
        let t = ((cursor.y - (bottom - EDGE)) / EDGE).clamp(0.0, 1.0);
        scroll.nudge(t * MAX_SPEED);
    }
}

/// Write the dropped arrangement into the saved ranking. The panel picks it up
/// on the next frame, because the ranking's revision feeds the rebuild signature.
fn record_drop(
    order: &mut InspectorSectionOrder,
    roots: &Query<&InspectorSectionRoot>,
    sections: &[Entity],
    dragged: Entity,
    at: usize,
) {
    let Ok(moved) = roots.get(dragged) else {
        return;
    };
    let mut ids: Vec<String> = sections
        .iter()
        .filter(|e| **e != dragged)
        .filter_map(|e| roots.get(*e).ok())
        .map(|r| r.type_id.to_string())
        .collect();
    ids.insert(at.min(ids.len()), moved.type_id.to_string());
    order.record(&ids);
}

/// Show the drop line at `top`, or hide it with `None`. Writes only on a real
/// change: any `DerefMut` on a `Node` re-runs layout for that subtree.
fn show_line(lines: &Query<Entity, With<SectionDropLine>>, nodes: &mut Query<&mut Node>, top: Option<Val>) {
    let Some(line) = lines.iter().next() else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(line) else {
        return;
    };
    match top {
        Some(top) => {
            if node.display != Display::Flex {
                node.display = Display::Flex;
            }
            if node.top != top {
                node.top = top;
            }
        }
        None => {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
    }
}

/// The line lives in the component list, so it scrolls with it — and so a panel
/// rebuild despawns it along with the sections. It is respawned on the next
/// frame of the drag rather than kept alive elsewhere, which is why nothing here
/// has to survive a rebuild.
fn spawn_line(commands: &mut Commands, container: Entity, top: Val) {
    let line = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(2.0),
                right: Val::Px(2.0),
                top,
                height: Val::Px(LINE_H),
                border_radius: BorderRadius::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            // Never a hover target: it sits on top of the headers it points
            // between.
            Pickable::IGNORE,
            SectionDropLine,
            Name::new("inspector-drop-line"),
        ))
        .id();
    // Appended last, which keeps it out of the way of `stripe_collapsed_headers`
    // — that derives a section's zebra stripe from its index in this list.
    commands.entity(container).add_child(line);
}

/// A node's top-left corner and size in logical window pixels, directly
/// comparable with `Window::cursor_position()`.
///
/// `UiGlobalTransform`, not `GlobalTransform`: bevy 0.19's layout writes the
/// node's placement there (its *centre*, in physical px), and a UI node's
/// `GlobalTransform` is left at the origin.
fn node_rect(e: Entity, geom: &Query<(&UiGlobalTransform, &ComputedNode)>) -> Option<(Vec2, Vec2)> {
    let (xf, node) = geom.get(e).ok()?;
    let inv = node.inverse_scale_factor();
    let size = node.size() * inv;
    Some((xf.translation * inv - size * 0.5, size))
}
