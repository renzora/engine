//! 2D viewport interaction — click-to-pick, drag-to-move, selection outline.
//!
//! Sibling of `entity_pick_system`/`gizmo_drag` for 3D, but much simpler:
//! the editor 2D camera is orthographic, so picking is just an AABB hit
//! test in world space and dragging is a cursor-to-world offset that
//! makes the entity snap to the cursor exactly. All gated on
//! `viewport_view == Two`.
//!
//! The selection outline is registered with `ViewportOverlayRegistry` so
//! it paints on top of the rendered image alongside the existing 3D
//! overlays.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{Node2d, PlayModeState};
use renzora_editor::EditorSelection;

/// Per-drag bookkeeping so cursor-to-entity tracking stays exact across
/// frames. Captured on drag start, cleared on release.
#[derive(Resource, Default)]
pub struct Drag2dState {
    /// Offset from cursor world position to entity local translation,
    /// captured at drag start. `None` while not dragging.
    offset: Option<Vec2>,
    /// Entity whose offset is captured. Recapture if the selection changes
    /// mid-drag (shouldn't happen, but defensive).
    entity: Option<Entity>,
}

/// Look up the editor 2D camera via the entity reference cached by
/// [`crate::light_gizmo::update_editor_camera_2d_cache`]. The overlay
/// drawer runs with `&World`, which can't initialize a `QueryState`, so
/// we lean on the cache the same way the 3D scene-icon overlay does.
fn find_editor_camera_2d<'a>(world: &'a World) -> Option<(&'a Camera, &'a GlobalTransform)> {
    let entity = world
        .get_resource::<crate::light_gizmo::SceneIconCache>()?
        .editor_camera_2d?;
    let camera = world.get::<Camera>(entity)?;
    let cam_gt = world.get::<GlobalTransform>(entity)?;
    Some((camera, cam_gt))
}

/// Convert a window-space cursor position into 2D world space via the
/// active editor 2D camera, if the cursor is over the viewport panel.
fn cursor_to_world(
    cursor_window: Vec2,
    viewport: &ViewportState,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<Vec2> {
    // Reject cursors outside the panel rect.
    let in_rect = cursor_window - viewport.screen_position;
    if in_rect.x < 0.0
        || in_rect.y < 0.0
        || in_rect.x >= viewport.screen_size.x
        || in_rect.y >= viewport.screen_size.y
    {
        return None;
    }

    // The camera renders into the offscreen image at `current_size`; the
    // panel displays that image stretched to `screen_size`. Cursor coords
    // need to be in image space for `viewport_to_world_2d` to give a
    // correct result.
    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return None;
    }
    let scaled = Vec2::new(
        in_rect.x * image_size.x / viewport.screen_size.x,
        in_rect.y * image_size.y / viewport.screen_size.y,
    );
    camera.viewport_to_world_2d(cam_gt, scaled).ok()
}

/// AABB pick: left-click sets `EditorSelection` to the topmost (highest Z)
/// Sprite under the cursor, or `None` if empty space.
pub fn pick_2d_system(
    selection: Res<EditorSelection>,
    mouse: Res<ButtonInput<MouseButton>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    sprites: Query<(Entity, &Sprite, &GlobalTransform)>,
) {
    if play_mode.map_or(false, |pm| pm.is_in_play_mode()) {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(viewport) = viewport else { return };
    if !viewport.hovered {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };
    let Some(world_pos) = cursor_to_world(cursor, &viewport, camera, cam_gt) else {
        return;
    };

    // AABB hit test sprites; topmost (highest Z) wins so layered scenes
    // pick the foreground sprite first.
    let mut best: Option<(Entity, f32)> = None;
    for (entity, sprite, gt) in &sprites {
        let Some(size) = sprite.custom_size else {
            // No custom_size and no image bound — nothing to pick.
            continue;
        };
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        let pos = gt.translation();
        let half = size * 0.5;
        if (world_pos.x - pos.x).abs() <= half.x && (world_pos.y - pos.y).abs() <= half.y {
            match best {
                None => best = Some((entity, pos.z)),
                Some((_, z)) if pos.z > z => best = Some((entity, pos.z)),
                _ => {}
            }
        }
    }

    selection.set(best.map(|(e, _)| e));
}

/// Left-drag translates the selected entity. Uses an offset-based scheme
/// (capture cursor↔entity offset on drag start, snap entity to cursor +
/// offset every frame) so the entity tracks the cursor exactly at any
/// pan/zoom and away from the viewport center.
pub fn drag_move_2d_system(
    selection: Res<EditorSelection>,
    mouse: Res<ButtonInput<MouseButton>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mut drag: ResMut<Drag2dState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    mut transforms: Query<&mut Transform>,
) {
    let cancel = |drag: &mut Drag2dState| {
        drag.offset = None;
        drag.entity = None;
    };

    if play_mode.map_or(false, |pm| pm.is_in_play_mode()) {
        cancel(&mut drag);
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        cancel(&mut drag);
        return;
    }
    let Some(viewport) = viewport else {
        cancel(&mut drag);
        return;
    };
    if !viewport.hovered {
        cancel(&mut drag);
        return;
    }
    let Some(entity) = selection.get() else {
        cancel(&mut drag);
        return;
    };
    let Ok(window) = windows.single() else {
        cancel(&mut drag);
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        cancel(&mut drag);
        return;
    };
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        cancel(&mut drag);
        return;
    };
    let Some(cursor_world) = cursor_to_world(cursor, &viewport, camera, cam_gt) else {
        cancel(&mut drag);
        return;
    };

    // Capture the cursor↔entity offset on drag start so the entity stays
    // pinned to where the user grabbed it (instead of snapping its origin
    // to the cursor on first move).
    if drag.offset.is_none() || drag.entity != Some(entity) {
        let Ok(tr) = transforms.get(entity) else {
            cancel(&mut drag);
            return;
        };
        drag.offset = Some(tr.translation.truncate() - cursor_world);
        drag.entity = Some(entity);
    }
    let offset = drag.offset.unwrap();

    let Ok(mut tr) = transforms.get_mut(entity) else {
        return;
    };
    tr.translation.x = cursor_world.x + offset.x;
    tr.translation.y = cursor_world.y + offset.y;
}

/// Egui overlay: paint a yellow rectangle outline + corner handles
/// around the selected 2D entity. Reads `Transform` (not GlobalTransform)
/// for parentless entities so the outline tracks the drag without one
/// frame of propagation lag.
pub fn draw_selection_outline_2d(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    if view != ViewportView::Two {
        return;
    }

    let Some(selection) = world.get_resource::<EditorSelection>() else {
        return;
    };
    let Some(entity) = selection.get() else {
        return;
    };
    let Some(viewport) = world.get_resource::<ViewportState>() else {
        return;
    };
    let Some((camera, cam_gt)) = find_editor_camera_2d(world) else {
        return;
    };

    // Prefer Transform over GlobalTransform when the entity has no parent —
    // the drag system writes Transform, and GlobalTransform doesn't update
    // until transform_propagate runs (which is after this overlay frame),
    // so reading GlobalTransform shows last frame's position.
    let pos = if world.get::<ChildOf>(entity).is_some() {
        let Some(gt) = world.get::<GlobalTransform>(entity) else {
            return;
        };
        gt.translation()
    } else {
        let Some(tr) = world.get::<Transform>(entity) else {
            return;
        };
        tr.translation
    };

    // Build a world-space AABB for the selection.
    let half = if let Some(sprite) = world.get::<Sprite>(entity) {
        let size = sprite.custom_size.unwrap_or(Vec2::ZERO);
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        size * 0.5
    } else if world.get::<Node2d>(entity).is_some() {
        // Empty group — small handle rect so the user has visual feedback.
        Vec2::splat(20.0)
    } else {
        return;
    };

    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return;
    }
    let to_screen = |world_pos: Vec3| -> Option<egui::Pos2> {
        let img = camera.world_to_viewport(cam_gt, world_pos).ok()?;
        let x = rect.min.x + img.x * rect.width() / image_size.x;
        let y = rect.min.y + img.y * rect.height() / image_size.y;
        Some(egui::Pos2::new(x, y))
    };

    // Project the four corners individually so a tilted parent transform
    // (when we eventually support rotated 2D groups) gives a correct quad.
    // Today everything's axis-aligned so it just falls out as a rect.
    let tl = to_screen(Vec3::new(pos.x - half.x, pos.y + half.y, pos.z));
    let tr = to_screen(Vec3::new(pos.x + half.x, pos.y + half.y, pos.z));
    let bl = to_screen(Vec3::new(pos.x - half.x, pos.y - half.y, pos.z));
    let br = to_screen(Vec3::new(pos.x + half.x, pos.y - half.y, pos.z));
    let (Some(tl), Some(tr), Some(bl), Some(br)) = (tl, tr, bl, br) else {
        return;
    };

    let outline_color = egui::Color32::from_rgb(250, 220, 100);
    let painter = ui.painter_at(rect);

    // Outline rect (axis-aligned bbox of the four projected corners).
    let outline = egui::Rect::from_two_pos(tl.min(bl), tr.max(br));
    painter.rect_stroke(
        outline,
        0.0,
        egui::Stroke::new(2.0, outline_color),
        egui::StrokeKind::Outside,
    );

    // Corner handles — small filled squares with a darker outline. Resize
    // interaction is a follow-up; right now they're purely visual.
    let handle_size = 8.0_f32;
    let half_handle = handle_size * 0.5;
    let handle_fill = egui::Color32::from_rgb(255, 255, 255);
    let handle_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50));
    for corner in [tl, tr, bl, br] {
        let h_rect = egui::Rect::from_center_size(
            corner,
            egui::Vec2::new(handle_size, handle_size),
        );
        painter.rect_filled(h_rect, 1.0, handle_fill);
        painter.rect_stroke(h_rect, 1.0, handle_stroke, egui::StrokeKind::Inside);
        let _ = half_handle;
    }
}
