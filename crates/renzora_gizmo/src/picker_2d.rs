//! 2D viewport interaction — click-to-pick, drag-to-move, resize handles,
//! and selection outline.
//!
//! Shape: on left-click the picker decides whether the user grabbed a
//! resize handle of the currently-selected sprite, the body of any
//! sprite, or empty space — and stores that decision in [`Drag2dState`].
//! [`drag_move_2d_system`] then executes Move or Resize each frame until
//! the mouse is released.
//!
//! All work is gated on `viewport_view == Two`. The selection outline is
//! registered with `ViewportOverlayRegistry` so it paints on top of the
//! rendered image alongside the existing 3D overlays.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{Node2d, PlayModeState};
use renzora_editor::EditorSelection;

/// Hit threshold for handle picking, in panel pixels. Generous so users
/// don't have to be sub-pixel accurate on a small sprite.
const HANDLE_HIT_RADIUS: f32 = 8.0;
/// Drawn handle size in panel pixels. Smaller than the hit radius so
/// users can grab a handle even if their cursor is just outside the
/// drawn square.
const HANDLE_DRAW_SIZE: f32 = 9.0;

/// Which resize handle the user grabbed. Indexed by which corner / edge
/// of the entity AABB the handle sits at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResizeHandle {
    NW,
    N,
    NE,
    W,
    E,
    SW,
    S,
    SE,
}

impl ResizeHandle {
    /// Per-frame resize: given the cursor's current world position and the
    /// AABB captured at drag start, returns `(new_translation, new_size)`
    /// for the entity. The opposite handle stays anchored.
    fn resize(self, cursor: Vec2, init_min: Vec2, init_max: Vec2) -> (Vec2, Vec2) {
        let (mut min_x, mut max_x, mut min_y, mut max_y) =
            (init_min.x, init_max.x, init_min.y, init_max.y);
        match self {
            ResizeHandle::NW => {
                min_x = cursor.x;
                max_y = cursor.y;
            }
            ResizeHandle::N => {
                max_y = cursor.y;
            }
            ResizeHandle::NE => {
                max_x = cursor.x;
                max_y = cursor.y;
            }
            ResizeHandle::W => {
                min_x = cursor.x;
            }
            ResizeHandle::E => {
                max_x = cursor.x;
            }
            ResizeHandle::SW => {
                min_x = cursor.x;
                min_y = cursor.y;
            }
            ResizeHandle::S => {
                min_y = cursor.y;
            }
            ResizeHandle::SE => {
                max_x = cursor.x;
                min_y = cursor.y;
            }
        }
        // Allow flipping the box so dragging past the opposite handle
        // mirrors instead of going negative on the size.
        let (lo_x, hi_x) = if min_x <= max_x {
            (min_x, max_x)
        } else {
            (max_x, min_x)
        };
        let (lo_y, hi_y) = if min_y <= max_y {
            (min_y, max_y)
        } else {
            (max_y, min_y)
        };
        // Floor at 1 world unit so the sprite stays visible / pickable.
        let size = Vec2::new((hi_x - lo_x).max(1.0), (hi_y - lo_y).max(1.0));
        let translation = Vec2::new((lo_x + hi_x) * 0.5, (lo_y + hi_y) * 0.5);
        (translation, size)
    }
}

/// What the active drag is doing, plus enough captured state to do it
/// without drift.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DragMode {
    #[default]
    None,
    /// Translate the entity. `offset` is `entity_translation - cursor_world`
    /// captured at drag start so the entity stays pinned to the grab point.
    Move {
        offset: Vec2,
    },
    /// Resize via one of the eight handles. Initial bounds are captured
    /// at drag start so the math doesn't accumulate float drift.
    Resize {
        handle: ResizeHandle,
        init_min: Vec2,
        init_max: Vec2,
    },
}

#[derive(Resource, Default)]
pub struct Drag2dState {
    pub mode: DragMode,
    pub entity: Option<Entity>,
}

impl Drag2dState {
    fn cancel(&mut self) {
        self.mode = DragMode::None;
        self.entity = None;
    }
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
    let in_rect = cursor_window - viewport.screen_position;
    if in_rect.x < 0.0
        || in_rect.y < 0.0
        || in_rect.x >= viewport.screen_size.x
        || in_rect.y >= viewport.screen_size.y
    {
        return None;
    }
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

/// Project a world-space point to panel-rect-relative pixel coords.
/// Returns `None` if the point is behind the camera.
fn world_to_panel(
    world_pos: Vec3,
    viewport: &ViewportState,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<Vec2> {
    let img = camera.world_to_viewport(cam_gt, world_pos).ok()?;
    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return None;
    }
    Some(Vec2::new(
        img.x * viewport.screen_size.x / image_size.x,
        img.y * viewport.screen_size.y / image_size.y,
    ))
}

/// Resolve the entity's effective 2D position. Reads `Transform` for
/// parentless entities so the overlay tracks an active drag without one
/// frame of `transform_propagate` lag.
fn entity_translation(world: &World, entity: Entity) -> Option<Vec3> {
    if world.get::<ChildOf>(entity).is_some() {
        Some(world.get::<GlobalTransform>(entity)?.translation())
    } else {
        Some(world.get::<Transform>(entity)?.translation)
    }
}

/// AABB half-extents for the selection. Sprites use `custom_size`,
/// `Node2d` groups get a small handle rectangle so empty groups have
/// visible feedback.
fn entity_half_size(world: &World, entity: Entity) -> Option<Vec2> {
    if let Some(sprite) = world.get::<Sprite>(entity) {
        let size = sprite_effective_size(world, sprite)?;
        if size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        Some(size * 0.5)
    } else if world.get::<Node2d>(entity).is_some() {
        Some(Vec2::splat(20.0))
    } else {
        None
    }
}

/// A sprite's effective render size, in world units. Falls back from
/// `custom_size` (explicit override) → `rect.size()` (sub-region of an
/// atlas) → the loaded image's native pixel dimensions. Returns `None`
/// while the image is still loading or if no image is bound — in
/// either case there's nothing meaningful to hit-test against.
pub fn sprite_effective_size(world: &World, sprite: &Sprite) -> Option<Vec2> {
    let images = world.get_resource::<Assets<Image>>()?;
    sprite_size_from_query(sprite, images)
}

/// System-friendly variant of `sprite_effective_size` — takes
/// `Res<Assets<Image>>` directly instead of going through `&World`,
/// so it can be called from systems that already have the asset
/// resource.
pub fn sprite_size_from_query(sprite: &Sprite, images: &Assets<Image>) -> Option<Vec2> {
    if let Some(custom) = sprite.custom_size {
        return Some(custom);
    }
    if let Some(rect) = sprite.rect {
        return Some(rect.size());
    }
    let image = images.get(&sprite.image)?;
    Some(image.size_f32())
}

/// Sample the sprite's alpha at a world position. Returns `None` if
/// the image isn't loaded (caller should fall back to AABB-only),
/// or if the cursor lands outside the sprite's UV space. Honours
/// `flip_x` / `flip_y` and `rect` (atlas sub-region) so picking
/// matches what the user actually sees on screen.
fn sample_sprite_alpha(
    sprite: &Sprite,
    images: &Assets<Image>,
    cursor_world: Vec2,
    sprite_world: Vec2,
    sprite_size: Vec2,
) -> Option<f32> {
    let image = images.get(&sprite.image)?;
    let img_size = image.size_f32();
    if img_size.x <= 0.0 || img_size.y <= 0.0 {
        return None;
    }
    if sprite_size.x <= 0.0 || sprite_size.y <= 0.0 {
        return None;
    }

    // Local position relative to sprite centre, then UV (0..1).
    // World Y is up, texture Y is down — flip the V axis.
    let local = cursor_world - sprite_world;
    let mut u = local.x / sprite_size.x + 0.5;
    let mut v = 0.5 - local.y / sprite_size.y;

    if sprite.flip_x {
        u = 1.0 - u;
    }
    if sprite.flip_y {
        v = 1.0 - v;
    }

    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }

    // UV → texture pixel. If `rect` is set the sprite renders only a
    // sub-region of the texture; map UV into that rect.
    let (tex_x, tex_y) = if let Some(rect) = sprite.rect {
        let rw = rect.max.x - rect.min.x;
        let rh = rect.max.y - rect.min.y;
        (rect.min.x + u * rw, rect.min.y + v * rh)
    } else {
        (u * img_size.x, v * img_size.y)
    };

    // Clamp to valid pixel index (size_f32 returns the full extent;
    // valid indices are 0..=size-1).
    let max_x = (img_size.x as u32).saturating_sub(1);
    let max_y = (img_size.y as u32).saturating_sub(1);
    let px = (tex_x as u32).min(max_x);
    let py = (tex_y as u32).min(max_y);

    image.get_color_at(px, py).ok().map(|c| c.alpha())
}

/// All eight handle positions for the entity's AABB, in world space.
fn handle_world_positions(translation: Vec2, half: Vec2) -> [(ResizeHandle, Vec2); 8] {
    let cx = translation.x;
    let cy = translation.y;
    [
        (ResizeHandle::NW, Vec2::new(cx - half.x, cy + half.y)),
        (ResizeHandle::N, Vec2::new(cx, cy + half.y)),
        (ResizeHandle::NE, Vec2::new(cx + half.x, cy + half.y)),
        (ResizeHandle::W, Vec2::new(cx - half.x, cy)),
        (ResizeHandle::E, Vec2::new(cx + half.x, cy)),
        (ResizeHandle::SW, Vec2::new(cx - half.x, cy - half.y)),
        (ResizeHandle::S, Vec2::new(cx, cy - half.y)),
        (ResizeHandle::SE, Vec2::new(cx + half.x, cy - half.y)),
    ]
}

/// On left-click, resolve what the user grabbed: a handle of the
/// currently-selected entity, the body of any sprite, or empty space.
/// Updates `EditorSelection` and `Drag2dState` accordingly.
pub fn pick_2d_system(
    selection: Res<EditorSelection>,
    mouse: Res<ButtonInput<MouseButton>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    images: Res<Assets<Image>>,
    mut drag: ResMut<Drag2dState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    sprites: Query<(Entity, &Sprite, &GlobalTransform)>,
    transforms: Query<&Transform>,
    world_root: Query<()>,
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
    let Some(cursor_world) = cursor_to_world(cursor, &viewport, camera, cam_gt) else {
        return;
    };
    let cursor_panel = cursor - viewport.screen_position;
    let _ = world_root;

    // --- Step 1: handle hit-test on the *currently selected* entity. ---
    if let Some(entity) = selection.get() {
        let translation_3d = transforms.get(entity).map(|t| t.translation).ok();
        let half = sprites
            .get(entity)
            .ok()
            .and_then(|(_, s, _)| sprite_size_from_query(s, &images))
            .map(|s| s * 0.5);
        if let (Some(t3d), Some(half)) = (translation_3d, half) {
            let translation = t3d.truncate();
            let init_min = translation - half;
            let init_max = translation + half;
            for (handle, world_pos) in handle_world_positions(translation, half) {
                let Some(panel_pos) =
                    world_to_panel(Vec3::new(world_pos.x, world_pos.y, t3d.z), &viewport, camera, cam_gt)
                else {
                    continue;
                };
                if (panel_pos - cursor_panel).length() <= HANDLE_HIT_RADIUS {
                    drag.entity = Some(entity);
                    drag.mode = DragMode::Resize {
                        handle,
                        init_min,
                        init_max,
                    };
                    return;
                }
            }
        }
    }

    // --- Step 2: AABB hit-test against all sprites, then alpha test. ---
    // Pixel-perfect picking: a sprite with a transparent pixel at the
    // click position shouldn't grab the click — let it fall through to
    // whatever's behind. Alpha == 0.0 → skipped. If the image isn't
    // loaded yet (alpha is `None`), fall back to the AABB hit so the
    // sprite isn't unclickable while loading.
    let mut best: Option<(Entity, f32, Vec2)> = None;
    for (entity, sprite, gt) in &sprites {
        let Some(size) = sprite_size_from_query(sprite, &images) else {
            continue;
        };
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        let pos = gt.translation();
        let half = size * 0.5;
        let aabb_hit = (cursor_world.x - pos.x).abs() <= half.x
            && (cursor_world.y - pos.y).abs() <= half.y;
        if !aabb_hit {
            continue;
        }
        if let Some(alpha) = sample_sprite_alpha(sprite, &images, cursor_world, pos.truncate(), size) {
            if alpha <= 0.0 {
                continue;
            }
        }
        match best {
            None => best = Some((entity, pos.z, pos.truncate())),
            Some((_, z, _)) if pos.z > z => best = Some((entity, pos.z, pos.truncate())),
            _ => {}
        }
    }

    if let Some((entity, _, translation)) = best {
        selection.set(Some(entity));
        drag.entity = Some(entity);
        drag.mode = DragMode::Move {
            offset: translation - cursor_world,
        };
    } else {
        selection.set(None);
        drag.cancel();
    }
}

/// Execute the active drag: Move snaps `Transform.translation` to
/// `cursor_world + offset`; Resize recomputes size + translation from
/// the captured AABB and the moving cursor. Releases drag when the
/// mouse button is released or the cursor leaves the viewport.
pub fn drag_move_2d_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mut drag: ResMut<Drag2dState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    mut transforms: Query<&mut Transform>,
    mut sprites: Query<&mut Sprite>,
) {
    if play_mode.map_or(false, |pm| pm.is_in_play_mode()) {
        drag.cancel();
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        drag.cancel();
        return;
    }
    let Some(viewport) = viewport else {
        drag.cancel();
        return;
    };
    if !viewport.hovered {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let DragMode::Move { offset } = drag.mode else {
        // Resize handled below; None means nothing to do.
        if let DragMode::Resize {
            handle,
            init_min,
            init_max,
        } = drag.mode
        {
            return resize_step(
                handle, init_min, init_max, shift, drag.entity, &viewport, &windows,
                &cameras_2d, &mut transforms, &mut sprites,
            );
        }
        return;
    };
    let Some(entity) = drag.entity else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };
    let Some(cursor_world) = cursor_to_world(cursor, &viewport, camera, cam_gt) else {
        return;
    };

    let Ok(mut tr) = transforms.get_mut(entity) else {
        return;
    };
    tr.translation.x = cursor_world.x + offset.x;
    tr.translation.y = cursor_world.y + offset.y;
}

#[allow(clippy::too_many_arguments)]
fn resize_step(
    handle: ResizeHandle,
    init_min: Vec2,
    init_max: Vec2,
    aspect_lock: bool,
    entity: Option<Entity>,
    viewport: &ViewportState,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras_2d: &Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    transforms: &mut Query<&mut Transform>,
    sprites: &mut Query<&mut Sprite>,
) {
    let Some(entity) = entity else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };
    let Some(cursor_world) = cursor_to_world(cursor, viewport, camera, cam_gt) else {
        return;
    };
    let (mut new_translation, mut new_size) = handle.resize(cursor_world, init_min, init_max);

    // Shift+drag on a corner handle locks the aspect ratio to the
    // initial size's. Edge handles are 1D resizes — aspect lock isn't
    // meaningful, so we leave them alone.
    let is_corner = matches!(
        handle,
        ResizeHandle::NW | ResizeHandle::NE | ResizeHandle::SW | ResizeHandle::SE
    );
    if aspect_lock && is_corner {
        let initial = init_max - init_min;
        if initial.x > f32::EPSILON && initial.y > f32::EPSILON {
            let aspect = initial.x / initial.y;
            let new_aspect = new_size.x / new_size.y.max(f32::EPSILON);
            if new_aspect > aspect {
                new_size.x = new_size.y * aspect;
            } else {
                new_size.y = new_size.x / aspect;
            }
            // Recompute the center so the *opposite* corner stays
            // anchored to its initial position — same invariant as the
            // un-locked resize, just with the locked size.
            let anchor = match handle {
                ResizeHandle::NW => Vec2::new(init_max.x, init_min.y),
                ResizeHandle::NE => Vec2::new(init_min.x, init_min.y),
                ResizeHandle::SW => Vec2::new(init_max.x, init_max.y),
                ResizeHandle::SE => Vec2::new(init_min.x, init_max.y),
                _ => unreachable!(),
            };
            let half = new_size * 0.5;
            new_translation = match handle {
                ResizeHandle::NW => Vec2::new(anchor.x - half.x, anchor.y + half.y),
                ResizeHandle::NE => Vec2::new(anchor.x + half.x, anchor.y + half.y),
                ResizeHandle::SW => Vec2::new(anchor.x - half.x, anchor.y - half.y),
                ResizeHandle::SE => Vec2::new(anchor.x + half.x, anchor.y - half.y),
                _ => unreachable!(),
            };
        }
    }

    if let Ok(mut tr) = transforms.get_mut(entity) {
        tr.translation.x = new_translation.x;
        tr.translation.y = new_translation.y;
    }
    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.custom_size = Some(new_size);
    }
}

/// Arrow-key nudge for the selected 2D entity. 1 unit per press, 10
/// units when Shift is held. Gated on viewport hover (so typing in
/// inspector text fields doesn't move the selection) and 2D view via
/// the system's run-condition.
pub fn keyboard_nudge_2d(
    selection: Res<EditorSelection>,
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    mut transforms: Query<&mut Transform>,
) {
    if play_mode.map_or(false, |pm| pm.is_in_play_mode()) {
        return;
    }
    if !viewport.map_or(false, |v| v.hovered) {
        return;
    }
    let Some(entity) = selection.get() else { return };

    let mut delta = Vec2::ZERO;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        delta.x -= 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        delta.x += 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        delta.y -= 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        delta.y += 1.0;
    }
    if delta == Vec2::ZERO {
        return;
    }

    let multiplier = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        10.0
    } else {
        1.0
    };
    delta *= multiplier;

    let Ok(mut tr) = transforms.get_mut(entity) else {
        return;
    };
    tr.translation.x += delta.x;
    tr.translation.y += delta.y;
}

/// Egui overlay: paint a yellow rectangle outline + 8 handles around the
/// selected 2D entity.
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
    let Some(translation_3d) = entity_translation(world, entity) else {
        return;
    };
    let Some(half) = entity_half_size(world, entity) else {
        return;
    };

    let translation = translation_3d.truncate();
    let z = translation_3d.z;

    let to_screen = |world_pos: Vec3| -> Option<egui::Pos2> {
        let panel = world_to_panel(world_pos, viewport, camera, cam_gt)?;
        Some(egui::Pos2::new(rect.min.x + panel.x, rect.min.y + panel.y))
    };

    // Outline corners.
    let tl = to_screen(Vec3::new(translation.x - half.x, translation.y + half.y, z));
    let tr_corner = to_screen(Vec3::new(translation.x + half.x, translation.y + half.y, z));
    let bl = to_screen(Vec3::new(translation.x - half.x, translation.y - half.y, z));
    let br = to_screen(Vec3::new(translation.x + half.x, translation.y - half.y, z));
    let (Some(tl), Some(tr_corner), Some(bl), Some(br)) = (tl, tr_corner, bl, br) else {
        return;
    };

    let outline_color = egui::Color32::from_rgb(250, 220, 100);
    let painter = ui.painter_at(rect);

    let outline = egui::Rect::from_two_pos(tl.min(bl), tr_corner.max(br));
    painter.rect_stroke(
        outline,
        0.0,
        egui::Stroke::new(2.0, outline_color),
        egui::StrokeKind::Outside,
    );

    // Handles: 4 corners + 4 edge midpoints. Drawn after the outline so
    // they paint on top of the rectangle stroke. Each carries the
    // direction it represents so we can pick the right resize cursor on
    // hover.
    let handle_fill = egui::Color32::from_rgb(255, 255, 255);
    let handle_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50));
    let edge_n = to_screen(Vec3::new(translation.x, translation.y + half.y, z));
    let edge_s = to_screen(Vec3::new(translation.x, translation.y - half.y, z));
    let edge_w = to_screen(Vec3::new(translation.x - half.x, translation.y, z));
    let edge_e = to_screen(Vec3::new(translation.x + half.x, translation.y, z));
    let mut handles: Vec<(ResizeHandle, egui::Pos2)> = vec![
        (ResizeHandle::NW, tl),
        (ResizeHandle::NE, tr_corner),
        (ResizeHandle::SW, bl),
        (ResizeHandle::SE, br),
    ];
    if let (Some(n), Some(s), Some(w), Some(e)) = (edge_n, edge_s, edge_w, edge_e) {
        handles.extend_from_slice(&[
            (ResizeHandle::N, n),
            (ResizeHandle::S, s),
            (ResizeHandle::W, w),
            (ResizeHandle::E, e),
        ]);
    }

    // Cursor hint: if the pointer is over a handle, set the matching
    // resize cursor so users feel the affordance before they click.
    if let Some(cursor) = ui.ctx().pointer_hover_pos() {
        for (handle, pos) in &handles {
            if (*pos - cursor).length() <= HANDLE_HIT_RADIUS {
                let icon = match handle {
                    ResizeHandle::NW | ResizeHandle::SE => egui::CursorIcon::ResizeNwSe,
                    ResizeHandle::NE | ResizeHandle::SW => egui::CursorIcon::ResizeNeSw,
                    ResizeHandle::N | ResizeHandle::S => egui::CursorIcon::ResizeVertical,
                    ResizeHandle::E | ResizeHandle::W => egui::CursorIcon::ResizeHorizontal,
                };
                ui.ctx().set_cursor_icon(icon);
                break;
            }
        }
    }

    for (_, pos) in handles {
        let h_rect = egui::Rect::from_center_size(
            pos,
            egui::Vec2::new(HANDLE_DRAW_SIZE, HANDLE_DRAW_SIZE),
        );
        painter.rect_filled(h_rect, 1.0, handle_fill);
        painter.rect_stroke(h_rect, 1.0, handle_stroke, egui::StrokeKind::Inside);
    }
}
