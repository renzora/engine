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
}

pub(crate) fn snapshot_widgets(
    mut state: ResMut<NativeCanvasState>,
    ui_scale: Option<Res<UiScale>>,
    widgets: Query<(Entity, &UiWidget, &ComputedNode, &UiGlobalTransform, Option<&ChildOf>)>,
    parents: Query<&ChildOf>,
) {
    state.widgets.clear();
    let Some(active) = state.active_canvas else { return };
    let scale = ui_scale.map(|s| s.0).unwrap_or(1.0).max(0.001);
    for (entity, widget, cn, ugt, child_of) in &widgets {
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
