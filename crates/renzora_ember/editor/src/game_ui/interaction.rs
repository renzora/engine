//! Canvas interaction: click-to-select, drag-to-move, resize (8 handles),
//! rotate, and marquee (rubber-band) multi-select. Everything is computed in
//! **design space** from the hit layer's `RelativeCursorPosition` (`normalized`
//! is centered −0.5..0.5; design px = `(norm + 0.5) * reference`). bevy_ui keeps
//! `normalized` populated even when the cursor is *outside* the frame (the value
//! just runs past ±0.5), so the same mapping covers marquees that start on the
//! dark background around the canvas. Angle/edge math is zoom-invariant.
//!
//! Press/drag model (matches what users expect from a 2D editor):
//!
//! - A press starts a *pending* gesture — nothing moves yet. Only once the
//!   cursor travels past [`DRAG_THRESHOLD`] does it become a real move/marquee.
//!   So clicking a widget no longer nudges it (a click has zero travel).
//! - Selection only *changes* on a click that didn't turn into a drag, so
//!   dragging never reselects something mid-gesture.
//! - Pressing inside the **currently selected** entity (its box, or any
//!   descendant) drags *that* entity — grab a selected root and drag it as a
//!   whole even though a child sits under the cursor. A plain click there still
//!   drills into the child.
//! - A drag that starts on empty canvas, or anywhere on the dark background
//!   around the frame, draws a marquee and selects the widgets it encloses.
//!
//! Write-back is **parent-relative**: `Node` percentages resolve against the
//! parent, so a nested widget's left/top/width/height are expressed as a percent
//! of its parent's design box — not the canvas reference. Doing it against the
//! reference squashed nested widgets (a button row inside a 360px card became a
//! percent of 1280). A move also pins width/height so popping a flex child out of
//! its parent's layout doesn't collapse it to content size.

use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, UiTransform};

use renzora::{EditorSelection, SplashState};

use crate::game_ui::geometry::{is_descendant_of, topmost_at};
use crate::game_ui::overlay::{CanvasHandle, CanvasHitLayer, HandleKind, ResizeHandle};
use crate::game_ui::NativeCanvasState;

/// Design-space pixels the cursor must travel after a press before the gesture
/// is treated as a drag (rather than a click).
const DRAG_THRESHOLD: f32 = 4.0;

/// Axis-aligned box `(x, y, w, h)` in design space.
type Bbox = (f32, f32, f32, f32);

/// Marker on the canvas viewport background (the dark area *around* the design
/// frame). A click there clears the selection; a drag there starts a marquee.
#[derive(Component)]
pub(crate) struct CanvasBackground;

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        canvas_interact
            .run_if(in_state(SplashState::Editor))
            // Play mode runs *inside* the editor (SplashState stays `Editor`), so
            // without this guard drag-to-move/resize/rotate would keep editing the
            // live UI layout while the user is playtesting. Mirrors the 3D gizmo.
            .run_if(renzora::not_in_play_mode),
    );
}

enum Mode {
    /// Pressed, not yet past the threshold. `select` is what a click selects;
    /// `drag` is what a drag moves; `marquee` starts a rubber-band instead.
    Pending { select: Option<Entity>, drag: Option<Entity>, marquee: bool, start_cursor: Vec2, start_bbox: Bbox },
    Move { entity: Entity, start_cursor: Vec2, start_bbox: Bbox },
    Resize { entity: Entity, handle: ResizeHandle, start_cursor: Vec2, bbox: Bbox },
    Rotate { entity: Entity, center: Vec2, start_offset: f32 },
    Marquee { start: Vec2 },
}

#[allow(clippy::too_many_arguments)]
fn canvas_interact(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut active: Local<Option<Mode>>,
    hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>,
    handles: Query<(&Interaction, &CanvasHandle)>,
    background: Query<&Interaction, With<CanvasBackground>>,
    parents: Query<&ChildOf>,
    mut state: ResMut<NativeCanvasState>,
    selection: Option<Res<EditorSelection>>,
    mut commands: Commands,
) {
    let Some(selection) = selection else { return };
    let Ok(rcp) = hit.single() else {
        if mouse.just_released(MouseButton::Left) {
            *active = None;
            state.marquee = None;
        }
        return;
    };
    // Design-space cursor — valid even past the frame edge (normalized runs
    // beyond ±0.5 when the cursor is over the background).
    let cursor = rcp
        .normalized
        .map(|n| Vec2::new((n.x + 0.5) * state.canvas_width, (n.y + 0.5) * state.canvas_height));

    // ── Release ── finalize regardless of where the cursor ended up.
    if mouse.just_released(MouseButton::Left) {
        match active.take() {
            // Never became a drag → a click: apply its (possibly empty) selection.
            Some(Mode::Pending { select, .. }) => {
                selection.set(select);
            }
            // Marquee: select everything fully enclosed by the rubber-band.
            Some(Mode::Marquee { start }) => {
                if let Some(end) = cursor {
                    let (min, max) = (start.min(end), start.max(end));
                    let enclosed: Vec<Entity> = state
                        .widgets
                        .iter()
                        .filter(|g| g.x >= min.x && g.y >= min.y && g.x + g.width <= max.x && g.y + g.height <= max.y)
                        .map(|g| g.entity)
                        .collect();
                    if enclosed.is_empty() {
                        selection.set(None);
                    } else {
                        selection.set_multiple(enclosed);
                    }
                }
            }
            _ => {}
        }
        state.marquee = None;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *active = None;
        state.marquee = None;
        return;
    }

    let Some(cursor) = cursor else { return };

    // ── Begin ──
    if mouse.just_pressed(MouseButton::Left) {
        if let Some((_, handle)) = handles.iter().find(|(i, _)| **i == Interaction::Pressed) {
            // A grab handle takes precedence and acts immediately.
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
            return;
        }

        // Over the design frame? Use the **geometric** `cursor_over` (reliable
        // regardless of focus/blocking) so a press on a widget always runs the
        // widget path and is never mistaken for a background marquee.
        if rcp.cursor_over {
            let hit_e = topmost_at(&state.widgets, cursor.x, cursor.y);
            if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
                // Ctrl-click toggles the widget in the multi-selection (no drag).
                if let Some(e) = hit_e {
                    selection.toggle(e);
                }
                *active = None;
                return;
            }

            // Pressing within the current selection (its box or a descendant of
            // it) drags the selection; a click still drills to the child below.
            let current = selection.get();
            let within_sel = match (current, hit_e) {
                (Some(sel), Some(h)) => sel == h || is_descendant_of(&parents, h, sel) || bbox_contains(&state, sel, cursor),
                (Some(sel), None) => bbox_contains(&state, sel, cursor),
                _ => false,
            };
            let drag = if within_sel { current } else { hit_e };
            // Empty canvas (no widget, outside the selection) → marquee.
            let marquee = !within_sel && hit_e.is_none();
            if !within_sel {
                selection.set(hit_e);
            }
            let start_bbox = drag
                .and_then(|e| state.widgets.iter().find(|g| g.entity == e))
                .map(|g| (g.x, g.y, g.width, g.height))
                .unwrap_or((cursor.x, cursor.y, 0.0, 0.0));
            *active = Some(Mode::Pending { select: hit_e, drag, marquee, start_cursor: cursor, start_bbox });
            return;
        }

        // Outside the frame: a press on the dark background starts a marquee (a
        // click there just deselects). Anywhere else (another panel) is ignored.
        if background.iter().any(|i| *i == Interaction::Pressed) {
            *active = Some(Mode::Pending { select: None, drag: None, marquee: true, start_cursor: cursor, start_bbox: (cursor.x, cursor.y, 0.0, 0.0) });
        } else {
            *active = None;
        }
        return;
    }

    // ── Promote a pending press once the cursor actually travels ──
    let promote = match active.as_ref() {
        Some(Mode::Pending { drag, marquee, start_cursor, start_bbox, .. })
            if (cursor - *start_cursor).length() > DRAG_THRESHOLD =>
        {
            Some((*drag, *marquee, *start_cursor, *start_bbox))
        }
        _ => None,
    };
    if let Some((drag, marquee, start_cursor, start_bbox)) = promote {
        *active = if marquee {
            Some(Mode::Marquee { start: start_cursor })
        } else if let Some(entity) = drag {
            selection.set(Some(entity));
            Some(Mode::Move { entity, start_cursor, start_bbox })
        } else {
            None
        };
    }

    // ── Apply ──
    let grid = if state.snap_enabled { state.grid_size } else { 0.0 };
    match active.as_ref() {
        Some(Mode::Move { entity, start_cursor, start_bbox }) => {
            let (bx, by, bw, bh) = *start_bbox;
            let nx = snap(bx + cursor.x - start_cursor.x, grid);
            let ny = snap(by + cursor.y - start_cursor.y, grid);
            let e = *entity;
            let p = parent_rect(&state, e);
            commands.queue(move |w: &mut World| set_node_move(w, e, nx, ny, bw, bh, p));
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
            let p = parent_rect(&state, e);
            commands.queue(move |w: &mut World| set_node_rect(w, e, nx, ny, nw, nh, p));
        }
        Some(Mode::Rotate { entity, center, start_offset }) => {
            let rot = (cursor.y - center.y).atan2(cursor.x - center.x) - start_offset;
            let e = *entity;
            commands.queue(move |w: &mut World| set_rotation(w, e, rot));
        }
        Some(Mode::Marquee { start }) => {
            state.marquee = Some((*start, cursor));
        }
        Some(Mode::Pending { .. }) | None => {}
    }
}

/// Whether `cursor` (design space) is inside entity `e`'s current geometry box.
fn bbox_contains(state: &NativeCanvasState, e: Entity, cursor: Vec2) -> bool {
    state
        .widgets
        .iter()
        .find(|g| g.entity == e)
        .map(|g| cursor.x >= g.x && cursor.x <= g.x + g.width && cursor.y >= g.y && cursor.y <= g.y + g.height)
        .unwrap_or(false)
}

/// The design-space box of `e`'s parent — the basis a `Node` percentage resolves
/// against. Falls back to the whole canvas when the parent isn't a tracked
/// widget (i.e. the canvas root itself).
fn parent_rect(state: &NativeCanvasState, e: Entity) -> Bbox {
    let parent = state.widgets.iter().find(|g| g.entity == e).and_then(|g| g.parent);
    parent
        .and_then(|p| state.widgets.iter().find(|g| g.entity == p))
        .map(|g| (g.x, g.y, g.width, g.height))
        .unwrap_or((0.0, 0.0, state.canvas_width, state.canvas_height))
}

fn snap(v: f32, grid: f32) -> f32 {
    if grid > 0.0 {
        (v / grid).round() * grid
    } else {
        v
    }
}

/// Move write-back, parent-relative. Pops the widget to absolute at the dragged
/// position *and* pins its current size — without that a flex child snaps to its
/// content size the instant it leaves its parent's layout (the "drag a full-
/// height bar and it squashes" bug), and a full-size node loses its 100%.
fn set_node_move(world: &mut World, entity: Entity, nx: f32, ny: f32, nw: f32, nh: f32, parent: Bbox) {
    let (px, py, pw, ph) = (parent.0, parent.1, parent.2.max(1.0), parent.3.max(1.0));
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if let Some(mut node) = em.get_mut::<Node>() {
            node.left = Val::Percent((nx - px) / pw * 100.0);
            node.top = Val::Percent((ny - py) / ph * 100.0);
            node.width = Val::Percent(nw / pw * 100.0);
            node.height = Val::Percent(nh / ph * 100.0);
            node.position_type = PositionType::Absolute;
        }
    }
}

fn set_node_rect(world: &mut World, entity: Entity, nx: f32, ny: f32, nw: f32, nh: f32, parent: Bbox) {
    let (px, py, pw, ph) = (parent.0, parent.1, parent.2.max(1.0), parent.3.max(1.0));
    let flex = renzora_ember::game_ui::spawn::is_flex_child(world, entity);
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if let Some(mut node) = em.get_mut::<Node>() {
            node.width = Val::Percent(nw / pw * 100.0);
            node.height = Val::Percent(nh / ph * 100.0);
            if !flex {
                node.left = Val::Percent((nx - px) / pw * 100.0);
                node.top = Val::Percent((ny - py) / ph * 100.0);
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
