//! 2D viewport interaction — click-to-pick, drag-to-move, resize handles,
//! a rotate handle, and the viewport cursor.
//!
//! Shape: on left-click the picker decides whether the user grabbed the
//! rotate handle or a resize handle of the currently-selected sprite, the
//! body of any sprite, or empty space — and stores that decision in
//! [`Drag2dState`]. [`drag_move_2d_system`] then executes Move / Resize /
//! Rotate each frame until the mouse is released.
//!
//! Rotation-aware throughout: hit-tests, alpha sampling, and resize all work
//! in the sprite's LOCAL frame (cursor rotated by the inverse of the entity's
//! z rotation), so a rotated sprite picks and resizes exactly along its own
//! axes. The captured resize bounds are local half-extents; only the final
//! translation is rotated back into world space.
//!
//! [`update_cursor_2d`] separately publishes the hover cursor each frame
//! (Move over a selected body, directional resize over handles, grab over the
//! rotate handle) via the shared [`ViewportCursorRequest`] — ember's cursor
//! system consumes it, keeping a single writer of the window `CursorIcon`.
//!
//! All work is gated on `viewport_view == Two`.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy::window::{PrimaryWindow, SystemCursorIcon};
use renzora::core::viewport_types::{
    ViewportCursorRequest, ViewportSettings, ViewportState, ViewportView,
};
use renzora::core::{Node2d, PlayModeState};
use renzora_editor_framework::EditorSelection;

/// Hit threshold for handle picking, in panel pixels. Generous so users
/// don't have to be sub-pixel accurate on a small sprite.
const HANDLE_HIT_RADIUS: f32 = 8.0;
/// Distance of the rotate handle above the selection's top-center handle,
/// in panel pixels. Matches the overlay drawing in `native_overlay_2d`.
pub const ROTATE_HANDLE_OFFSET_PX: f32 = 22.0;

/// Which resize handle the user grabbed. Named by which corner / edge
/// of the entity's LOCAL box the handle sits at (N = local +Y).
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
    /// Per-frame resize in the sprite's LOCAL frame: given the cursor's local
    /// position and the local bounds captured at drag start, returns
    /// `(new_local_center, new_size)`. The opposite handle stays anchored.
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

    /// The handle's outward direction in the entity's LOCAL frame (N = +Y).
    fn local_dir(self) -> Vec2 {
        match self {
            ResizeHandle::NW => Vec2::new(-1.0, 1.0),
            ResizeHandle::N => Vec2::new(0.0, 1.0),
            ResizeHandle::NE => Vec2::new(1.0, 1.0),
            ResizeHandle::W => Vec2::new(-1.0, 0.0),
            ResizeHandle::E => Vec2::new(1.0, 0.0),
            ResizeHandle::SW => Vec2::new(-1.0, -1.0),
            ResizeHandle::S => Vec2::new(0.0, -1.0),
            ResizeHandle::SE => Vec2::new(1.0, -1.0),
        }
    }

    /// The directional resize cursor for this handle on an entity rotated by
    /// `angle`. World +Y is screen-up, so a pure-N handle at zero rotation is
    /// a NS resize; rotation shifts the direction through the four diagonal /
    /// axis cursor buckets (45° sectors).
    fn cursor(self, angle: f32) -> SystemCursorIcon {
        let world = (self.local_dir().to_angle() + angle).to_degrees();
        // Fold to 0..180 (a resize axis is direction-less) and bucket.
        let folded = world.rem_euclid(180.0);
        if folded < 22.5 || folded >= 157.5 {
            SystemCursorIcon::EwResize
        } else if folded < 67.5 {
            // Up-right axis on screen (world y-up) = bottom-left↔top-right.
            SystemCursorIcon::NeswResize
        } else if folded < 112.5 {
            SystemCursorIcon::NsResize
        } else {
            SystemCursorIcon::NwseResize
        }
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
    Move { offset: Vec2 },
    /// Resize via one of the eight handles. Bounds are LOCAL half-extent
    /// boxes captured at drag start (so the math doesn't accumulate float
    /// drift); `center`/`angle` are the entity pose at drag start, used to
    /// map the cursor into the local frame and the result back out.
    Resize {
        handle: ResizeHandle,
        init_min: Vec2,
        init_max: Vec2,
        center: Vec2,
        angle: f32,
    },
    /// Rotate via the handle above the selection. `start_angle` is the
    /// entity's z rotation and `grab_angle` the cursor's polar angle around
    /// the entity at drag start; the delta between the live cursor angle and
    /// `grab_angle` is added to `start_angle`, so the handle follows the
    /// cursor without snapping to it.
    Rotate { start_angle: f32, grab_angle: f32 },
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

/// The entity's z rotation in radians. 2D scenes only rotate about z, so the
/// XYZ euler decomposition's z component is exact for editor-authored poses.
fn rotation_z(q: Quat) -> f32 {
    q.to_euler(EulerRot::XYZ).2
}

/// Look up the editor 2D camera via the entity reference cached by
/// [`crate::light_gizmo::update_editor_camera_2d_cache`]. The overlay
/// drawer runs with `&World`, which can't initialize a `QueryState`, so
/// we lean on the cache the same way the 3D scene-icon overlay does.
fn find_editor_camera_2d(world: &World) -> Option<(&Camera, &GlobalTransform)> {
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

/// Sample the sprite's alpha at a LOCAL position (cursor relative to the
/// sprite centre, already rotated into the sprite's frame). Returns `None`
/// if the image isn't loaded (caller should fall back to box-only), or if
/// the position lands outside the sprite's UV space. Honours `flip_x` /
/// `flip_y` and `rect` (atlas sub-region) so picking matches what the user
/// actually sees on screen.
fn sample_sprite_alpha(
    sprite: &Sprite,
    images: &Assets<Image>,
    local: Vec2,
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

/// All eight handle positions for the entity's (possibly rotated) box, in
/// world space.
fn handle_world_positions(
    translation: Vec2,
    half: Vec2,
    angle: f32,
) -> [(ResizeHandle, Vec2); 8] {
    let rot = Rot2::radians(angle);
    let at = |x: f32, y: f32| translation + rot * Vec2::new(x, y);
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

/// Panel-space position of the rotate handle: [`ROTATE_HANDLE_OFFSET_PX`]
/// pixels beyond the top-center resize handle, along the box's (rotated)
/// local up direction. Computed in panel space so the offset is a constant
/// on-screen distance at any zoom.
fn rotate_handle_panel_pos(
    center: Vec2,
    half: Vec2,
    angle: f32,
    z: f32,
    viewport: &ViewportState,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<Vec2> {
    let top_center = center + Rot2::radians(angle) * Vec2::new(0.0, half.y);
    let c = world_to_panel(center.extend(z), viewport, camera, cam_gt)?;
    let t = world_to_panel(top_center.extend(z), viewport, camera, cam_gt)?;
    let dir = (t - c).normalize_or_zero();
    if dir == Vec2::ZERO {
        return None;
    }
    Some(t + dir * ROTATE_HANDLE_OFFSET_PX)
}

/// What the cursor is over, resolved for both click handling and the hover
/// cursor: the selected entity's rotate handle, one of its resize handles,
/// or nothing handle-shaped (body picking is separate).
enum HandleHit {
    Rotate,
    Resize(ResizeHandle),
}

/// Hit-test the selected entity's rotate + resize handles at a panel-space
/// cursor position. The rotate handle wins ties — it sits beyond the box, so
/// overlap only happens on tiny selections where rotate is the harder grab.
#[allow(clippy::too_many_arguments)]
fn hit_test_handles(
    translation: Vec3,
    half: Vec2,
    angle: f32,
    cursor_panel: Vec2,
    viewport: &ViewportState,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<HandleHit> {
    let center = translation.truncate();
    if let Some(pos) =
        rotate_handle_panel_pos(center, half, angle, translation.z, viewport, camera, cam_gt)
    {
        if (pos - cursor_panel).length() <= HANDLE_HIT_RADIUS {
            return Some(HandleHit::Rotate);
        }
    }
    for (handle, world_pos) in handle_world_positions(center, half, angle) {
        let Some(panel_pos) = world_to_panel(
            Vec3::new(world_pos.x, world_pos.y, translation.z),
            viewport,
            camera,
            cam_gt,
        ) else {
            continue;
        };
        if (panel_pos - cursor_panel).length() <= HANDLE_HIT_RADIUS {
            return Some(HandleHit::Resize(handle));
        }
    }
    None
}

/// On left-click, resolve what the user grabbed: the rotate handle or a
/// resize handle of the currently-selected entity, the body of any sprite,
/// or empty space. Updates `EditorSelection` and `Drag2dState` accordingly.
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
) {
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
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

    // --- Step 1: handle hit-test on the *currently selected* entity. ---
    if let Some(entity) = selection.get() {
        let tr = transforms.get(entity).ok();
        let half = sprites
            .get(entity)
            .ok()
            .and_then(|(_, s, _)| sprite_size_from_query(s, &images))
            .map(|s| s * 0.5);
        if let (Some(tr), Some(half)) = (tr, half) {
            let angle = rotation_z(tr.rotation);
            let translation = tr.translation.truncate();
            match hit_test_handles(
                tr.translation,
                half,
                angle,
                cursor_panel,
                &viewport,
                camera,
                cam_gt,
            ) {
                Some(HandleHit::Rotate) => {
                    drag.entity = Some(entity);
                    drag.mode = DragMode::Rotate {
                        start_angle: angle,
                        grab_angle: (cursor_world - translation).to_angle(),
                    };
                    return;
                }
                Some(HandleHit::Resize(handle)) => {
                    drag.entity = Some(entity);
                    drag.mode = DragMode::Resize {
                        handle,
                        init_min: -half,
                        init_max: half,
                        center: translation,
                        angle,
                    };
                    return;
                }
                None => {}
            }
        }
    }

    // --- Step 2: box hit-test against all sprites, then alpha test. ---
    // The test runs in each sprite's local frame, so rotated sprites pick
    // exactly along their drawn silhouette. Pixel-perfect picking: a sprite
    // with a transparent pixel at the click position shouldn't grab the
    // click — let it fall through to whatever's behind. Alpha == 0.0 →
    // skipped. If the image isn't loaded yet (alpha is `None`), fall back to
    // the box hit so the sprite isn't unclickable while loading.
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
        let local = Rot2::radians(-rotation_z(gt.rotation())) * (cursor_world - pos.truncate());
        if local.x.abs() > half.x || local.y.abs() > half.y {
            continue;
        }
        if let Some(alpha) = sample_sprite_alpha(sprite, &images, local, size) {
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

/// Snap a value to the nearest multiple of `grid`. Returns the value
/// unchanged when `grid <= 0`.
#[inline]
fn snap_to_grid(value: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        value
    } else {
        (value / grid).round() * grid
    }
}

/// Execute the active drag: Move snaps `Transform.translation` to
/// `cursor_world + offset`; Resize recomputes size + translation from the
/// captured local bounds and the cursor mapped into the sprite's frame;
/// Rotate turns the entity to follow the cursor's polar angle. When the
/// viewport's translate-snap toggle is on, Move snaps to its grid step so
/// sprites land on whole pixel / tile boundaries; Rotate snaps to the rotate
/// step (or 15° with Shift). Releases drag when the mouse button is released
/// or the cursor leaves the viewport.
pub fn drag_move_2d_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    settings: Option<Res<ViewportSettings>>,
    play_mode: Option<Res<PlayModeState>>,
    mut drag: ResMut<Drag2dState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    mut transforms: Query<&mut Transform>,
    mut sprites: Query<&mut Sprite>,
) {
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
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
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    match drag.mode {
        DragMode::None => {}
        DragMode::Move { offset } => {
            let Ok(mut tr) = transforms.get_mut(entity) else {
                return;
            };
            let mut new_x = cursor_world.x + offset.x;
            let mut new_y = cursor_world.y + offset.y;
            if let Some(s) = settings.as_deref() {
                if s.snap.translate_enabled {
                    new_x = snap_to_grid(new_x, s.snap.translate_snap);
                    new_y = snap_to_grid(new_y, s.snap.translate_snap);
                }
            }
            tr.translation.x = new_x;
            tr.translation.y = new_y;
        }
        DragMode::Resize {
            handle,
            init_min,
            init_max,
            center,
            angle,
        } => resize_step(
            handle,
            init_min,
            init_max,
            center,
            angle,
            shift,
            entity,
            cursor_world,
            &mut transforms,
            &mut sprites,
        ),
        DragMode::Rotate {
            start_angle,
            grab_angle,
        } => {
            let Ok(mut tr) = transforms.get_mut(entity) else {
                return;
            };
            let center = tr.translation.truncate();
            let cursor_angle = (cursor_world - center).to_angle();
            let mut angle = start_angle + (cursor_angle - grab_angle);
            // Shift always coarse-snaps; otherwise honour the viewport's
            // rotate-snap toggle (degrees, same as the 3D gizmo).
            let snap_deg = if shift {
                15.0
            } else {
                settings
                    .as_deref()
                    .filter(|s| s.snap.rotate_enabled)
                    .map(|s| s.snap.rotate_snap)
                    .unwrap_or(0.0)
            };
            if snap_deg > 0.0 {
                angle = (angle.to_degrees() / snap_deg).round() * snap_deg;
                angle = angle.to_radians();
            }
            tr.rotation = Quat::from_rotation_z(angle);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resize_step(
    handle: ResizeHandle,
    init_min: Vec2,
    init_max: Vec2,
    center: Vec2,
    angle: f32,
    aspect_lock: bool,
    entity: Entity,
    cursor_world: Vec2,
    transforms: &mut Query<&mut Transform>,
    sprites: &mut Query<&mut Sprite>,
) {
    // Map the cursor into the sprite's local frame captured at drag start;
    // all the box math below is rotation-free.
    let local_cursor = Rot2::radians(-angle) * (cursor_world - center);
    let (mut new_translation, mut new_size) = handle.resize(local_cursor, init_min, init_max);

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

    // Local center back to world space.
    let world_translation = center + Rot2::radians(angle) * new_translation;
    if let Ok(mut tr) = transforms.get_mut(entity) {
        tr.translation.x = world_translation.x;
        tr.translation.y = world_translation.y;
    }
    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.custom_size = Some(new_size);
    }
}

/// Publish the viewport cursor for the current hover / drag state:
/// grab over the rotate handle, a directional resize cursor over resize
/// handles, Move over the selected entity's body. Runs unconditionally in
/// the editor (no `in_two_view` gate) so it can CLEAR the request when the
/// pointer leaves the viewport or the view leaves 2D.
#[allow(clippy::too_many_arguments)]
pub fn update_cursor_2d(
    selection: Option<Res<EditorSelection>>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<PlayModeState>>,
    images: Res<Assets<Image>>,
    drag: Option<Res<Drag2dState>>,
    mut request: ResMut<ViewportCursorRequest>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    sprites: Query<(&Sprite, &GlobalTransform)>,
    transforms: Query<&Transform>,
) {
    let mut cursor_icon: Option<SystemCursorIcon> = None;
    // Deferred assignment (rather than early returns) so every bail-out path
    // still clears a stale request.
    'resolve: {
        let in_2d = settings.map(|s| s.viewport_view == ViewportView::Two).unwrap_or(false);
        let editing = !play_mode.is_some_and(|pm| pm.is_in_play_mode());
        if !in_2d || !editing {
            break 'resolve;
        }
        let Some(vs) = viewport else { break 'resolve };
        if !vs.hovered {
            break 'resolve;
        }

        // Mid-drag the cursor reflects the ACTION, not what's under the
        // pointer — a fast drag routinely outruns the handle.
        if let Some(drag) = drag.as_deref() {
            match drag.mode {
                DragMode::Move { .. } => {
                    cursor_icon = Some(SystemCursorIcon::Move);
                    break 'resolve;
                }
                DragMode::Rotate { .. } => {
                    cursor_icon = Some(SystemCursorIcon::Grabbing);
                    break 'resolve;
                }
                DragMode::Resize { handle, angle, .. } => {
                    cursor_icon = Some(handle.cursor(angle));
                    break 'resolve;
                }
                DragMode::None => {}
            }
        }

        let Ok(window) = windows.single() else { break 'resolve };
        let Some(cursor) = window.cursor_position() else {
            break 'resolve;
        };
        let Ok((camera, cam_gt)) = cameras_2d.single() else {
            break 'resolve;
        };
        let Some(cursor_world) = cursor_to_world(cursor, &vs, camera, cam_gt) else {
            break 'resolve;
        };
        let cursor_panel = cursor - vs.screen_position;

        let Some(entity) = selection.as_ref().and_then(|s| s.get()) else {
            break 'resolve;
        };
        let Ok(tr) = transforms.get(entity) else {
            break 'resolve;
        };
        let Some(size) = sprites
            .get(entity)
            .ok()
            .and_then(|(s, _)| sprite_size_from_query(s, &images))
        else {
            break 'resolve;
        };
        let half = size * 0.5;
        let angle = rotation_z(tr.rotation);

        match hit_test_handles(tr.translation, half, angle, cursor_panel, &vs, camera, cam_gt) {
            Some(HandleHit::Rotate) => cursor_icon = Some(SystemCursorIcon::Grab),
            Some(HandleHit::Resize(handle)) => cursor_icon = Some(handle.cursor(angle)),
            None => {
                // Over the selected sprite's body (rotated frame) → Move.
                let local = Rot2::radians(-angle) * (cursor_world - tr.translation.truncate());
                if local.x.abs() <= half.x && local.y.abs() <= half.y {
                    cursor_icon = Some(SystemCursorIcon::Move);
                }
            }
        }
    }

    if request.0 != cursor_icon {
        request.0 = cursor_icon;
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
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if !viewport.is_some_and(|v| v.hovered) {
        return;
    }
    let Some(entity) = selection.get() else {
        return;
    };

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
