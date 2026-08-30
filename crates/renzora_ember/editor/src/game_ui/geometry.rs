//! Per-frame snapshot of each widget's **design-space** rect, used by the
//! selection overlay + interaction. Design space == the offscreen render's
//! pixels (reference resolution), which is what the editor authors in.
//!
//! A widget's design rect comes from its laid-out `ComputedNode.size` +
//! `UiGlobalTransform.translation` (the node *center*), divided back out by
//! `UiScale` (1.0 in editor builds). This matches exactly how the egui canvas
//! computed handle positions.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use renzora_ember::game_ui::UiWidget;

use crate::game_ui::NativeCanvasState;

#[derive(Clone)]
pub(crate) struct WidgetGeom {
    pub entity: Entity,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub locked: bool,
    pub parent: Option<Entity>,
    /// Steps from the active canvas: `0` is the template root, `1` its
    /// children, and so on. The hit-test picks the deepest containing node —
    /// see [`topmost_at`].
    pub depth: u32,
    /// Whether this node is laid out by its parent's flex flow, rather than
    /// pinned with `position: absolute`.
    ///
    /// This is what decides which kind of drag it gets. A node in flow is
    /// *reordered* among its siblings; a node already pinned is moved freely,
    /// which is also the escape hatch — write `position="absolute"` in the
    /// markup and you get the old free placement back.
    pub in_flow: bool,
    /// Main axis of this node's own children: `true` for a row, `false` for a
    /// column. Used to decide whether an insertion point is left/right of a
    /// child or above/below it.
    pub row: bool,
}

/// A pending flow drop: which container would receive the node, and where in it.
#[derive(Clone, Copy)]
pub(crate) struct DropTarget {
    pub parent: Entity,
    /// The sibling the node lands in front of, or `None` to go last.
    ///
    /// An *entity* rather than an index on purpose. The index would have to be
    /// counted among the live children and then applied to the file's children,
    /// and those two lists are only incidentally the same — a tag that flattens
    /// into its host, or one that builds no entity at all, puts them out of step
    /// and the reorder lands in the wrong slot. Naming the sibling lets the
    /// writeback look up its own recorded position in the file and sidestep the
    /// correspondence entirely.
    pub before: Option<Entity>,
    /// The parent's design-space box, for the boundary the overlay draws.
    pub parent_box: (f32, f32, f32, f32),
    /// Insertion line in design space, as `(from, to)`.
    pub line: (Vec2, Vec2),
}

pub(crate) fn snapshot_widgets(
    mut state: ResMut<NativeCanvasState>,
    ui_scale: Option<Res<UiScale>>,
    widgets: Query<(Entity, &UiWidget, &ComputedNode, &UiGlobalTransform, &Node, Option<&ChildOf>)>,
    parents: Query<&ChildOf>,
) {
    state.widgets.clear();
    let Some(active) = state.active_canvas else { return };
    let scale = ui_scale.map(|s| s.0).unwrap_or(1.0).max(0.001);
    for (entity, widget, cn, ugt, node, child_of) in &widgets {
        // The canvas root is the surface, not something on it. It gets tagged
        // `UiWidget` like every other node once its template builds, and
        // `is_descendant_of` counts an entity as its own descendant, so without
        // this it joined the hit-test — a click on empty canvas grabbed the root
        // and a drag rewrote its `Node` to a zero-size box parked off-screen,
        // which renders as a canvas that is simply blank. There is nothing to
        // drag it *within*, so it is excluded rather than merely locked.
        if entity == active || !is_descendant_of(&parents, entity, active) {
            continue;
        }
        // Angle comes from the *global* transform, not the node's own
        // `UiTransform`. Rotation is inherited, so a `<text>` inside a rotated
        // `<button>` has no local rotation of its own — reading the local one
        // gave every child an angle of zero and drew an axis-aligned selection
        // box over a rotated widget. Position already came from the global
        // transform, so the box was centred correctly and only the angle was
        // wrong, which is exactly what it looked like.
        let (_, angle, translation) = ugt.to_scale_angle_translation();
        let w = cn.size.x / scale;
        let h = cn.size.y / scale;
        let cx = translation.x / scale;
        let cy = translation.y / scale;
        state.widgets.push(WidgetGeom {
            entity,
            x: cx - w * 0.5,
            y: cy - h * 0.5,
            width: w,
            height: h,
            rotation: angle,
            locked: widget.locked,
            parent: child_of.map(|c| c.parent()),
            depth: depth_below(&parents, entity, active),
            in_flow: node.position_type == PositionType::Relative,
            row: matches!(
                node.flex_direction,
                FlexDirection::Row | FlexDirection::RowReverse
            ),
        });
    }
}

/// Walk `ChildOf` upward from `e` looking for `ancestor`.
pub(crate) fn is_descendant_of(parents: &Query<&ChildOf>, mut e: Entity, ancestor: Entity) -> bool {
    for _ in 0..256 {
        if e == ancestor {
            return true;
        }
        match parents.get(e) {
            Ok(c) => e = c.parent(),
            Err(_) => return false,
        }
    }
    false
}

/// Track the widget under the cursor, for the hover name badge.
///
/// Separate from the interaction system's hit-test on purpose: that one runs
/// only on press and resolves "what does a press act on", which depends on the
/// current selection (pressing inside a selected node drags *it*, not the child
/// under the pointer). This is the plain question — what is the cursor over —
/// and the answer differs.
pub(crate) fn track_hover(
    mut state: ResMut<NativeCanvasState>,
    hit: Query<&bevy::ui::RelativeCursorPosition, With<crate::game_ui::overlay::CanvasHitLayer>>,
) {
    let want = hit
        .iter()
        .next()
        .filter(|r| r.cursor_over)
        .and_then(|r| r.normalized)
        .map(|n| {
            Vec2::new(
                (n.x + 0.5) * state.canvas_width,
                (n.y + 0.5) * state.canvas_height,
            )
        })
        .and_then(|c| topmost_at(&state.widgets, c.x, c.y));
    if state.hovered != want {
        state.hovered = want;
    }
}

/// Work out where a flow drag would drop `dragged` if released at `cursor`.
///
/// The container is chosen from the deepest widget under the cursor that is not
/// the dragged node or inside it — dropping a node into itself is the one move
/// that cannot be expressed. A leaf (a `<text>`, an `<icon>`) is not a container,
/// so the search continues up to its parent; that is what makes dropping "onto"
/// a button mean "next to that button" rather than "inside its label".
///
/// The index comes from comparing the cursor against the midpoints of the
/// container's existing children along its main axis, which is the rule every
/// list reorder uses and the only one that feels right when the thing you are
/// dragging is still occupying one of the slots.
pub(crate) fn drop_target_at(
    widgets: &[WidgetGeom],
    parents: &Query<&ChildOf>,
    dragged: Entity,
    cursor: Vec2,
) -> Option<DropTarget> {
    let contains = |g: &WidgetGeom| {
        cursor.x >= g.x && cursor.x <= g.x + g.width && cursor.y >= g.y && cursor.y <= g.y + g.height
    };
    // Deepest hit that is not the dragged subtree.
    let hit = widgets
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            contains(g) && g.entity != dragged && !is_descendant_of(parents, g.entity, dragged)
        })
        .max_by_key(|(i, g)| (g.depth, *i))
        .map(|(_, g)| g)?;

    // A node with no children of its own cannot receive one, so aim at its
    // parent and treat the hit as the sibling we are landing beside.
    let has_children = widgets.iter().any(|g| g.parent == Some(hit.entity));
    let parent = if has_children {
        hit
    } else {
        let p = hit.parent?;
        widgets.iter().find(|g| g.entity == p)?
    };
    if parent.entity == dragged || is_descendant_of(parents, parent.entity, dragged) {
        return None;
    }

    // Siblings in layout order along the main axis. The dragged node stays in
    // the list: it still occupies a slot, and removing it here would make the
    // index refer to a different arrangement than the file has.
    let mut siblings: Vec<&WidgetGeom> = widgets
        .iter()
        .filter(|g| g.parent == Some(parent.entity))
        .collect();
    if parent.row {
        siblings.sort_by(|a, b| a.x.total_cmp(&b.x));
    } else {
        siblings.sort_by(|a, b| a.y.total_cmp(&b.y));
    }

    let mid = |g: &WidgetGeom| {
        if parent.row {
            g.x + g.width * 0.5
        } else {
            g.y + g.height * 0.5
        }
    };
    let along = if parent.row { cursor.x } else { cursor.y };
    let index = siblings.iter().take_while(|g| along > mid(g)).count();

    // The insertion line spans the container across the main axis, drawn at the
    // boundary the node would land on.
    let at = match siblings.get(index) {
        Some(next) => {
            if parent.row {
                next.x
            } else {
                next.y
            }
        }
        None => match siblings.last() {
            Some(last) => {
                if parent.row {
                    last.x + last.width
                } else {
                    last.y + last.height
                }
            }
            None => {
                if parent.row {
                    parent.x
                } else {
                    parent.y
                }
            }
        },
    };
    let line = if parent.row {
        (
            Vec2::new(at, parent.y),
            Vec2::new(at, parent.y + parent.height),
        )
    } else {
        (
            Vec2::new(parent.x, at),
            Vec2::new(parent.x + parent.width, at),
        )
    };

    Some(DropTarget {
        parent: parent.entity,
        before: siblings.get(index).map(|g| g.entity),
        parent_box: (parent.x, parent.y, parent.width, parent.height),
        line,
    })
}

/// How many `ChildOf` steps `e` sits below `ancestor`. Returns 0 for the
/// ancestor itself, and for anything that never reaches it (the caller has
/// already filtered those out with [`is_descendant_of`]).
fn depth_below(parents: &Query<&ChildOf>, mut e: Entity, ancestor: Entity) -> u32 {
    for step in 0..256 {
        if e == ancestor {
            return step;
        }
        match parents.get(e) {
            Ok(c) => e = c.parent(),
            Err(_) => return step,
        }
    }
    256
}

/// The deepest non-locked widget whose AABB contains the design-space point.
///
/// Depth decides it, not position in the array. This used to search the list in
/// reverse on the assumption that later entries paint on top, but the list comes
/// out of a Bevy query, so its order is by *archetype* — arbitrary with respect
/// to the tree. The template root is a 100% × 100% node that contains every
/// point, so whenever it happened to sort after a `<button>`, clicking that
/// button selected the whole canvas instead. `<text>` and `<icon>` landed in
/// different archetypes and kept working, which is what made it look like
/// buttons specifically were unclickable.
///
/// Deepest-wins is also what the markup loader already promises: clicking a
/// `<text>` inside a `<panel>` should land on the text. Ties (siblings that
/// overlap, both at the same depth) fall back to the later entry.
pub(crate) fn topmost_at(widgets: &[WidgetGeom], px: f32, py: f32) -> Option<Entity> {
    widgets
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            !g.locked && px >= g.x && px <= g.x + g.width && py >= g.y && py <= g.y + g.height
        })
        .max_by_key(|(i, g)| (g.depth, *i))
        .map(|(_, g)| g.entity)
}
