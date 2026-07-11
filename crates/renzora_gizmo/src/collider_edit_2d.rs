//! 2D viewport collider editing — drag handles for the selected entity's
//! `CollisionShapeData` while the inspector's collider **Edit** toggle
//! (`ColliderEditMode.active`) is on.
//!
//! The 2D twin of `collider_handles` (the 3D sphere handles): same activation
//! flag, but hit-tested in panel space like the sprite selection frame. While
//! active the normal 2D picker stands down entirely (see the edit-mode gates
//! in `picker_2d`), so a click over the viewport edits the collider instead of
//! re-selecting or dragging the sprite — mirroring how the 3D transform gizmo
//! hides in edit mode. Dragging **inside** the shape moves its `offset`;
//! dragging a **handle** resizes (box edges/corners; circle and capsule radii
//! resize about the centre). One gesture = one undo step, diffed on release.
//!
//! The chrome (green outline + handles) is drawn by
//! `renzora_viewport::native_overlay_2d` beside the selection frame — that
//! crate owns all screen-space 2D chrome (gizmos can't sit above sprites).

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, SystemCursorIcon};

use renzora::core::viewport_types::{
    ViewportCursorRequest, ViewportSettings, ViewportState, ViewportView,
};
use renzora::core::PlayModeState;
use renzora_editor_framework::EditorSelection;
use renzora_physics::{ColliderEditMode, CollisionShapeData, CollisionShapeType};

use crate::picker_2d::{
    cursor_to_world, rotation_z, world_to_panel, ResizeHandle, HANDLE_HIT_RADIUS,
};

/// The collider's half-extent box in the ENTITY's local frame, per shape —
/// what the handles and the body hit-test key off. The overlay drawer in
/// `renzora_viewport` mirrors this mapping (the crates don't link).
pub fn collider_half(shape: &CollisionShapeData) -> Vec2 {
    match shape.shape_type {
        // A Mesh shape has no editable extents in 3D, but in 2D it *renders*
        // as the fitted box (see the avian2d backend), so edit that box.
        CollisionShapeType::Box | CollisionShapeType::Mesh => shape.half_extents.truncate(),
        CollisionShapeType::Sphere | CollisionShapeType::Cylinder => Vec2::splat(shape.radius),
        CollisionShapeType::Capsule => {
            Vec2::new(shape.radius, shape.half_height + shape.radius)
        }
    }
}

enum ColliderDragMode {
    /// Grabbed inside the shape: move `offset`. The grab point (local, relative
    /// to the shape centre) keeps the shape pinned under the cursor.
    Move { grab_local: Vec2 },
    /// Grabbed a handle: resize. Bounds are the shape's local box captured at
    /// drag start, so the math never accumulates drift.
    Resize {
        handle: ResizeHandle,
        init_min: Vec2,
        init_max: Vec2,
    },
}

struct ColliderDrag {
    entity: Entity,
    mode: ColliderDragMode,
    start: CollisionShapeData,
}

#[derive(Resource, Default)]
pub struct ColliderDrag2d(Option<ColliderDrag>);

/// Undo step for one collider gesture: the whole `CollisionShapeData` before
/// and after (a corner drag changes offset and extents together).
struct ColliderShape2dCmd {
    entity: Entity,
    old: CollisionShapeData,
    new: CollisionShapeData,
}

impl renzora_undo::UndoCommand for ColliderShape2dCmd {
    fn label(&self) -> &str {
        "Collider"
    }
    fn execute(&mut self, world: &mut World) {
        if let Some(mut s) = world.get_mut::<CollisionShapeData>(self.entity) {
            *s = self.new.clone();
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(mut s) = world.get_mut::<CollisionShapeData>(self.entity) {
            *s = self.old.clone();
        }
    }
}

/// Diff a finished gesture against its captured start and queue one undo step.
/// (`Query::get` yields read-only items even on a `&mut` query, so this borrows
/// the system's shape query immutably.)
fn record_collider_drag(
    drag: ColliderDrag,
    shapes: &Query<(&mut CollisionShapeData, &GlobalTransform)>,
    commands: &mut Commands,
) {
    let Ok((shape, _)) = shapes.get(drag.entity) else {
        return;
    };
    if *shape == drag.start {
        return;
    }
    let cmd = ColliderShape2dCmd {
        entity: drag.entity,
        old: drag.start,
        new: shape.clone(),
    };
    commands.queue(move |world: &mut World| {
        let ctx = renzora_undo::active_context(world);
        renzora_undo::record(world, ctx.clone(), Box::new(cmd));
        renzora_undo::seal(world, &ctx);
    });
}

/// Hit-test + drag for the selected collider, plus the hover cursor while the
/// edit mode owns the viewport. Runs only in 2D edit view; `picker_2d`'s
/// systems all stand down while `ColliderEditMode.active`, so this is the sole
/// consumer of the left button (and the sole cursor publisher) in that state.
#[allow(clippy::too_many_arguments)]
pub fn collider_edit_2d_system(
    edit_mode: Option<Res<ColliderEditMode>>,
    selection: Res<EditorSelection>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<ColliderDrag2d>,
    mut request: ResMut<ViewportCursorRequest>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    mut shapes: Query<(&mut CollisionShapeData, &GlobalTransform)>,
    mut commands: Commands,
) {
    let in_2d = settings
        .map(|s| s.viewport_view == ViewportView::Two)
        .unwrap_or(false);
    // While `ColliderEditMode.active`, this system OWNS the cursor request —
    // `picker_2d::update_cursor_2d` stands down completely (two conditional
    // writers, exclusive conditions). So even when the view is unusable (3D
    // view, play mode) it must clear its own stale request rather than bail.
    let owns = edit_mode.is_some_and(|m| m.active);
    let active = owns && in_2d && !play_mode.is_some_and(|pm| pm.is_in_play_mode());
    if !active {
        // Edit mode ended mid-drag (Esc / inspector toggle): the gesture is
        // over — still record it so the change isn't silently un-undoable.
        if let Some(d) = drag.0.take() {
            record_collider_drag(d, &shapes, &mut commands);
        }
        if owns && request.0.is_some() {
            request.0 = None;
        }
        return;
    }

    // Release ends the gesture whatever else this frame resolves to.
    if !mouse.pressed(MouseButton::Left) {
        if let Some(d) = drag.0.take() {
            record_collider_drag(d, &shapes, &mut commands);
        }
    }

    let mut cursor_icon: Option<SystemCursorIcon> = None;
    'resolve: {
        let Some(entity) = selection.get() else {
            break 'resolve;
        };
        let Ok((mut shape, gt)) = shapes.get_mut(entity) else {
            break 'resolve;
        };
        let Some(vs) = viewport.as_deref() else {
            break 'resolve;
        };
        let Ok(window) = windows.single() else {
            break 'resolve;
        };
        let Some(cursor) = window.cursor_position() else {
            break 'resolve;
        };
        let Ok((camera, cam_gt)) = cameras_2d.single() else {
            break 'resolve;
        };

        // Everything below works in the ENTITY's local frame (like the sprite
        // resize): cursor rotated by the inverse of the entity's z rotation,
        // shape centre at `offset.xy`. A rotated entity edits exactly along
        // its own collider axes.
        let entity_pos = gt.translation().truncate();
        let angle = rotation_z(gt.rotation());
        let rot = Rot2::radians(angle);
        let center_local = shape.offset.truncate();
        let half = collider_half(&shape);

        let cursor_world = cursor_to_world(cursor, vs, camera, cam_gt);
        let cursor_local = cursor_world.map(|w| rot.inverse() * (w - entity_pos));
        let cursor_panel = cursor - vs.screen_position;

        // Hover: nearest handle within the grab radius, else the body.
        let mut hovered: Option<ResizeHandle> = None;
        if drag.0.is_none() {
            for (handle, local) in handle_local_positions(center_local, half) {
                let world = entity_pos + rot * local;
                let Some(panel) = world_to_panel(
                    world.extend(gt.translation().z),
                    vs,
                    camera,
                    cam_gt,
                ) else {
                    continue;
                };
                if (panel - cursor_panel).length() <= HANDLE_HIT_RADIUS {
                    hovered = Some(handle);
                    break;
                }
            }
        }
        let inside_body = cursor_local.is_some_and(|l| {
            (l - center_local).abs().cmple(half).all()
        });

        // Press starts a gesture (only over the viewport, like the picker).
        if let (true, true, Some(local)) =
            (mouse.just_pressed(MouseButton::Left), vs.hovered, cursor_local)
        {
            if let Some(handle) = hovered {
                drag.0 = Some(ColliderDrag {
                    entity,
                    mode: ColliderDragMode::Resize {
                        handle,
                        init_min: center_local - half,
                        init_max: center_local + half,
                    },
                    start: shape.clone(),
                });
            } else if inside_body {
                drag.0 = Some(ColliderDrag {
                    entity,
                    mode: ColliderDragMode::Move {
                        grab_local: local - center_local,
                    },
                    start: shape.clone(),
                });
            }
        }

        // Drag update.
        if let (Some(d), Some(local), true) =
            (drag.0.as_ref(), cursor_local, mouse.pressed(MouseButton::Left))
        {
            if d.entity == entity {
                match &d.mode {
                    ColliderDragMode::Move { grab_local } => {
                        let new_center = local - *grab_local;
                        if shape.offset.truncate() != new_center {
                            shape.offset.x = new_center.x;
                            shape.offset.y = new_center.y;
                        }
                    }
                    ColliderDragMode::Resize {
                        handle,
                        init_min,
                        init_max,
                    } => {
                        apply_resize(&mut shape, &d.start, *handle, local, *init_min, *init_max);
                    }
                }
                cursor_icon = Some(match &d.mode {
                    ColliderDragMode::Move { .. } => SystemCursorIcon::Move,
                    ColliderDragMode::Resize { handle, .. } => handle.cursor(angle),
                });
                break 'resolve;
            }
        }

        // Hover cursor (no drag in flight).
        if vs.hovered {
            cursor_icon = hovered
                .map(|h| h.cursor(angle))
                .or(inside_body.then_some(SystemCursorIcon::Move));
        }
    }

    if request.0 != cursor_icon {
        request.0 = cursor_icon;
    }
}

/// The eight handle positions in the ENTITY's local frame (shape centre +
/// half-extent corners/edges). N = local +Y, matching `ResizeHandle`.
pub fn handle_local_positions(center: Vec2, half: Vec2) -> [(ResizeHandle, Vec2); 8] {
    let at = |x: f32, y: f32| center + Vec2::new(x, y);
    [
        (ResizeHandle::NW, at(-half.x, half.y)),
        (ResizeHandle::N, at(0.0, half.y)),
        (ResizeHandle::NE, at(half.x, half.y)),
        (ResizeHandle::W, at(-half.x, 0.0)),
        (ResizeHandle::E, at(half.x, 0.0)),
        (ResizeHandle::SW, at(-half.x, -half.y)),
        (ResizeHandle::S, at(0.0, -half.y)),
        (ResizeHandle::SE, at(half.x, -half.y)),
    ]
}

/// Apply a handle drag to the shape, per shape type. Boxes resize like the
/// sprite frame (opposite edge anchored, box may flip); circles and capsules
/// resize about their centre — their handles set radius / height directly, so
/// the shape stays symmetric the way the physics shape actually is.
fn apply_resize(
    shape: &mut CollisionShapeData,
    start: &CollisionShapeData,
    handle: ResizeHandle,
    cursor_local: Vec2,
    init_min: Vec2,
    init_max: Vec2,
) {
    let center = start.offset.truncate();
    let d = cursor_local - center;
    match shape.shape_type {
        CollisionShapeType::Box | CollisionShapeType::Mesh => {
            let (new_center, size) = handle.resize(cursor_local, init_min, init_max);
            shape.offset.x = new_center.x;
            shape.offset.y = new_center.y;
            shape.half_extents.x = (size.x * 0.5).max(0.01);
            shape.half_extents.y = (size.y * 0.5).max(0.01);
        }
        CollisionShapeType::Sphere | CollisionShapeType::Cylinder => {
            shape.radius = d.length().max(0.5);
        }
        CollisionShapeType::Capsule => match handle {
            // Side handles size the radius; top/bottom size the cylinder part
            // (total half-height = half_height + radius, hence the subtraction).
            ResizeHandle::W | ResizeHandle::E => shape.radius = d.x.abs().max(0.5),
            ResizeHandle::N | ResizeHandle::S => {
                shape.half_height = (d.y.abs() - start.radius).max(0.0)
            }
            _ => shape.radius = d.length().max(0.5),
        },
    }
}
