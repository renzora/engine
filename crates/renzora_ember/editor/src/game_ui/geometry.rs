//! Per-frame snapshot of each widget's **design-space** rect, used by the
//! selection overlay + interaction. Design space == the offscreen render's
//! pixels (reference resolution), which is what the editor authors in.
//!
//! A widget's design rect comes from its laid-out `ComputedNode.size` +
//! `UiGlobalTransform.translation` (the node *center*), divided back out by
//! `UiScale` (1.0 in editor builds). This matches exactly how the egui canvas
//! computed handle positions.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiTransform};

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
}

pub(crate) fn snapshot_widgets(
    mut state: ResMut<NativeCanvasState>,
    ui_scale: Option<Res<UiScale>>,
    widgets: Query<(Entity, &UiWidget, &ComputedNode, &UiGlobalTransform, Option<&UiTransform>, Option<&ChildOf>)>,
    parents: Query<&ChildOf>,
) {
    state.widgets.clear();
    let Some(active) = state.active_canvas else { return };
    let scale = ui_scale.map(|s| s.0).unwrap_or(1.0).max(0.001);
    for (entity, widget, cn, ugt, ut, child_of) in &widgets {
        if !is_descendant_of(&parents, entity, active) {
            continue;
        }
        let w = cn.size.x / scale;
        let h = cn.size.y / scale;
        let cx = ugt.translation.x / scale;
        let cy = ugt.translation.y / scale;
        state.widgets.push(WidgetGeom {
            entity,
            x: cx - w * 0.5,
            y: cy - h * 0.5,
            width: w,
            height: h,
            rotation: ut.map(|t| t.rotation.as_radians()).unwrap_or(0.0),
            locked: widget.locked,
            parent: child_of.map(|c| c.parent()),
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

/// Topmost non-locked widget whose AABB contains the design-space point.
/// Later entries paint on top, so search in reverse.
pub(crate) fn topmost_at(widgets: &[WidgetGeom], px: f32, py: f32) -> Option<Entity> {
    widgets
        .iter()
        .rev()
        .find(|g| !g.locked && px >= g.x && px <= g.x + g.width && py >= g.y && py <= g.y + g.height)
        .map(|g| g.entity)
}
