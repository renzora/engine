//! Canvas interaction: click-to-select, drag-to-move, resize (8 handles) and
//! rotate. Everything is computed in **design space** from the hit layer's
//! `RelativeCursorPosition` (`normalized` is centered −0.5..0.5; design px =
//! `(norm + 0.5) * reference`). Angle and edge math are zoom-invariant, so no
//! window-pixel / zoom bookkeeping is needed.
//!
//! Write-back matches the egui canvas: move/resize write `Node.left/top/width/
//! height` as a percentage of the reference resolution (position is skipped for
//! flex children, whose parent owns layout); rotate writes `UiTransform.rotation`.
//!
//! v1 simplifications vs egui: no scale-mode resize, no shift/alt/ctrl modifiers,
//! no marquee box-select, and resize ignores the widget's existing rotation/scale
//! (axis-aligned). Those refinements + align/distribute are follow-ups.

use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, UiTransform};

use renzora::{EditorSelection, SplashState};

use crate::geometry::topmost_at;
use crate::overlay::{CanvasHandle, CanvasHitLayer, HandleKind, ResizeHandle};
use crate::NativeCanvasState;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, canvas_interact.run_if(in_state(SplashState::Editor)));
}

enum Mode {
    Move { entity: Entity, start_cursor: Vec2, start_pos: Vec2 },
    Resize { entity: Entity, handle: ResizeHandle, start_cursor: Vec2, bbox: (f32, f32, f32, f32) },
    Rotate { entity: Entity, center: Vec2, start_offset: f32 },
}

fn canvas_interact(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut active: Local<Option<Mode>>,
    hit: Query<(&Interaction, &RelativeCursorPosition), With<CanvasHitLayer>>,
    handles: Query<(&Interaction, &CanvasHandle)>,
    state: Res<NativeCanvasState>,
    selection: Option<Res<EditorSelection>>,
    mut commands: Commands,
) {
    let Some(selection) = selection else { return };
    let Ok((interaction, rcp)) = hit.single() else { return };
    let Some(norm) = rcp.normalized else {
        if !mouse.pressed(MouseButton::Left) {
            *active = None;
        }
        return;
    };
    let cursor = Vec2::new((norm.x + 0.5) * state.canvas_width, (norm.y + 0.5) * state.canvas_height);

    // ── Begin ──
    if mouse.just_pressed(MouseButton::Left) {
        if let Some((_, handle)) = handles.iter().find(|(i, _)| **i == Interaction::Pressed) {
            if let Some(g) = state.widgets.iter().find(|g| g.entity == handle.widget) {
                *active = Some(match handle.kind {
                    HandleKind::Resize(rh) => Mode::Resize { entity: handle.widget, handle: rh, start_cursor: cursor, bbox: (g.x, g.y, g.width, g.height) },
                    HandleKind::Rotate => {
                        let center = Vec2::new(g.x + g.width * 0.5, g.y + g.height * 0.5);
                        let a = (cursor.y - center.y).atan2(cursor.x - center.x);
                        Mode::Rotate { entity: handle.widget, center, start_offset: a - g.rotation }
                    }
                });
            }
        } else if *interaction == Interaction::Pressed {
            let hit_e = topmost_at(&state.widgets, cursor.x, cursor.y);
            let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
            if ctrl {
                // Ctrl-click toggles the widget in the multi-selection (no drag).
                if let Some(e) = hit_e {
                    selection.toggle(e);
                }
                *active = None;
            } else {
                selection.set(hit_e);
                *active = hit_e.and_then(|e| state.widgets.iter().find(|g| g.entity == e).map(|g| Mode::Move { entity: e, start_cursor: cursor, start_pos: Vec2::new(g.x, g.y) }));
            }
        } else {
            *active = None;
        }
    }

    if !mouse.pressed(MouseButton::Left) {
        *active = None;
        return;
    }

    // ── Apply ──
    let (rw, rh) = (state.canvas_width.max(1.0), state.canvas_height.max(1.0));
    let grid = if state.snap_enabled { state.grid_size } else { 0.0 };
    match active.as_ref() {
        Some(Mode::Move { entity, start_cursor, start_pos }) => {
            let nx = snap(start_pos.x + cursor.x - start_cursor.x, grid);
            let ny = snap(start_pos.y + cursor.y - start_cursor.y, grid);
            let e = *entity;
            commands.queue(move |w: &mut World| set_node_pos(w, e, nx, ny, rw, rh));
        }
        Some(Mode::Resize { entity, handle, start_cursor, bbox }) => {
            let (l, t, r, b) = handle.sides();
            let dx = cursor.x - start_cursor.x;
            let dy = cursor.y - start_cursor.y;
            let nx = snap(bbox.0 + if l { dx } else { 0.0 }, grid);
            let ny = snap(bbox.1 + if t { dy } else { 0.0 }, grid);
            let nw = snap((bbox.2 + if r { dx } else { 0.0 } - if l { dx } else { 0.0 }).max(10.0), grid).max(10.0);
            let nh = snap((bbox.3 + if b { dy } else { 0.0 } - if t { dy } else { 0.0 }).max(10.0), grid).max(10.0);
            let e = *entity;
            commands.queue(move |w: &mut World| set_node_rect(w, e, nx, ny, nw, nh, rw, rh));
        }
        Some(Mode::Rotate { entity, center, start_offset }) => {
            let rot = (cursor.y - center.y).atan2(cursor.x - center.x) - start_offset;
            let e = *entity;
            commands.queue(move |w: &mut World| set_rotation(w, e, rot));
        }
        None => {}
    }
}

fn snap(v: f32, grid: f32) -> f32 {
    if grid > 0.0 {
        (v / grid).round() * grid
    } else {
        v
    }
}

fn set_node_pos(world: &mut World, entity: Entity, nx: f32, ny: f32, rw: f32, rh: f32) {
    // Body-drag always repositions the widget. If it was a flex child, setting
    // `position_type = Absolute` (below) pops it out of its parent's auto-layout
    // to the position the user dragged to — the conventional "drag to move freely
    // overrides auto-layout" behavior. Previously this early-returned for flex
    // children, so translate silently no-op'd while resize/rotate kept working.
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if let Some(mut node) = em.get_mut::<Node>() {
            node.left = Val::Percent(nx / rw * 100.0);
            node.top = Val::Percent(ny / rh * 100.0);
            node.position_type = PositionType::Absolute;
        }
    }
}

fn set_node_rect(world: &mut World, entity: Entity, nx: f32, ny: f32, nw: f32, nh: f32, rw: f32, rh: f32) {
    let flex = renzora_game_ui::spawn::is_flex_child(world, entity);
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if let Some(mut node) = em.get_mut::<Node>() {
            node.width = Val::Percent(nw / rw * 100.0);
            node.height = Val::Percent(nh / rh * 100.0);
            if !flex {
                node.left = Val::Percent(nx / rw * 100.0);
                node.top = Val::Percent(ny / rh * 100.0);
                node.position_type = PositionType::Absolute;
            }
        }
    }
}

fn set_rotation(world: &mut World, entity: Entity, rot: f32) {
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if em.get::<UiTransform>().is_none() {
            em.insert(UiTransform::IDENTITY);
        }
        if let Some(mut t) = em.get_mut::<UiTransform>() {
            t.rotation = Rot2::radians(rot);
        }
    }
}
