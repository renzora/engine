//! Canvas interaction: click-to-select (hit-test the design-space point under
//! the cursor) and drag-to-move the selected widget (writes `Node.left/top` as
//! a percentage of the reference resolution, matching the egui canvas + the
//! align/distribute write-back).
//!
//! Resize (the 8 handles), rotate, marquee box-select and align/distribute land
//! in follow-ups.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor::{EditorSelection, SplashState};

use crate::geometry::topmost_at;
use crate::overlay::CanvasHitLayer;
use crate::NativeCanvasState;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, canvas_interact.run_if(in_state(SplashState::Editor)));
}

/// Active drag: the dragged entity, the last cursor position, and the drag's
/// accumulated design-space top-left (so the drag is independent of the
/// per-frame snapshot).
type Drag = (Entity, Vec2, Vec2);

fn cursor(windows: &Query<&Window>) -> Option<Vec2> {
    windows.iter().next().and_then(|w| w.cursor_position())
}

fn canvas_interact(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<Drag>>,
    hit: Query<(&Interaction, &RelativeCursorPosition), With<CanvasHitLayer>>,
    state: Res<NativeCanvasState>,
    selection: Option<Res<EditorSelection>>,
    mut commands: Commands,
) {
    let Some(selection) = selection else { return };

    // ── Begin: press over the canvas → hit-test + select (+ maybe start drag).
    if mouse.just_pressed(MouseButton::Left) {
        let pressed = hit.iter().find(|(i, _)| **i == Interaction::Pressed);
        if let Some((_, rcp)) = pressed {
            if let Some(norm) = rcp.normalized {
                // Centered normalized (-0.5..0.5) → design px (0..reference).
                let px = (norm.x + 0.5) * state.canvas_width;
                let py = (norm.y + 0.5) * state.canvas_height;
                let hit_e = topmost_at(&state.widgets, px, py);
                selection.set(hit_e);
                *active = match (hit_e, cursor(&windows)) {
                    (Some(e), Some(c)) => state.widgets.iter().find(|g| g.entity == e).map(|g| (e, c, Vec2::new(g.x, g.y))),
                    _ => None,
                };
            }
        } else {
            *active = None;
        }
    }

    if !mouse.pressed(MouseButton::Left) {
        *active = None;
        return;
    }

    // ── Drag: move the active widget.
    let (Some((entity, last, design_pos)), Some(c)) = (*active, cursor(&windows)) else {
        return;
    };
    let delta = c - last;
    if delta == Vec2::ZERO {
        return;
    }
    let zoom = state.zoom.max(0.001);
    let new_pos = design_pos + delta / zoom;
    *active = Some((entity, c, new_pos));

    let (rw, rh) = (state.canvas_width.max(1.0), state.canvas_height.max(1.0));
    let (nx, ny) = (new_pos.x, new_pos.y);
    commands.queue(move |world: &mut World| {
        if let Ok(mut em) = world.get_entity_mut(entity) {
            if let Some(mut node) = em.get_mut::<Node>() {
                node.left = Val::Percent(nx / rw * 100.0);
                node.top = Val::Percent(ny / rh * 100.0);
                node.position_type = PositionType::Absolute;
            }
        }
    });
}
