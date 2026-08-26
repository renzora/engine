//! Renzora Camera — orbit camera controller for the editor viewport.
//!
//! Provides Blender/Unreal-style 3D navigation:
//! - Right-click + drag: look around (yaw/pitch)
//! - Right-click + WASD: fly movement
//! - Middle-click drag: orbit around focus point
//! - Alt + left-click drag: orbit around focus point
//! - Scroll wheel: dolly zoom (move along view direction)
//! - Shift: move faster (2x)
//! - Ctrl: move slower (0.25x)

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::{
    CameraOrbitSnapshot, NavOverlayState, ProjectionMode as VpProjectionMode, ViewportMode,
    ViewportSettings, ViewportState, ViewportView,
};
use renzora::core::InputFocusState;
use renzora::core::{EditorCamera, PlayModeCamera, ViewportCamera};
use renzora_editor_framework::EditorSelection;

/// Orbit camera state for the editor viewport.
///
/// Stored as a component on the `SceneCamera` entity so it persists in scene RON files.
/// Editor-only: the runtime/server won't register this type (stripped at export).
// Bevy 0.19: Resource: Component, so deriving both conflicts.
#[derive(Clone, Resource, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct OrbitCameraState {
    /// The point the camera orbits around.
    pub focus: Vec3,
    /// Distance from the focus point.
    pub distance: f32,
    /// Horizontal rotation angle (radians).
    pub yaw: f32,
    /// Vertical rotation angle (radians).
    pub pitch: f32,
    /// Camera projection mode.
    pub projection_mode: ProjectionMode,
}

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            distance: 4.5,
            yaw: 0.3,
            pitch: 0.4,
            projection_mode: ProjectionMode::Perspective,
        }
    }
}

impl OrbitCameraState {
    /// Calculate camera position from orbit parameters.
    pub fn calculate_position(&self) -> Vec3 {
        self.focus
            + Vec3::new(
                self.distance * self.pitch.cos() * self.yaw.sin(),
                self.distance * self.pitch.sin(),
                self.distance * self.pitch.cos() * self.yaw.cos(),
            )
    }

    /// Calculate camera transform from orbit parameters.
    pub fn calculate_transform(&self) -> Transform {
        Transform::from_translation(self.calculate_position()).looking_at(self.focus, Vec3::Y)
    }

    /// Focus on a specific point.
    pub fn focus_on(&mut self, point: Vec3) {
        self.focus = point;
    }

    /// Zoom by delta (positive = closer).
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).max(0.1);
    }

    /// Orbit by delta angles.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.5, 1.5);
    }

    /// Aim the orbit camera so it sits at `translation` looking along
    /// `rotation`'s forward (−Z), preserving the current orbit `distance`.
    ///
    /// Used by "go to camera preset" to drive the editor view to a saved angle.
    /// Roll is dropped — the orbit camera is always Y-up (matching
    /// [`Self::calculate_transform`], which `looking_at`s the focus). The focus
    /// is placed `distance` units ahead so subsequent orbit/zoom feel natural.
    pub fn set_from_view(&mut self, translation: Vec3, rotation: Quat) {
        let forward = (rotation * Vec3::NEG_Z).normalize_or_zero();
        if forward == Vec3::ZERO {
            return;
        }
        // position = focus + distance * u, with the camera looking toward focus,
        // so the focus→camera unit vector u = −forward.
        let u = -forward;
        self.pitch = u.y.clamp(-1.0, 1.0).asin().clamp(-1.5, 1.5);
        self.yaw = u.x.atan2(u.z);
        self.focus = translation - u * self.distance;
    }
}

/// Camera projection mode.
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, Default, Reflect, serde::Serialize, serde::Deserialize,
)]
pub enum ProjectionMode {
    #[default]
    Perspective,
    Orthographic,
}

impl ProjectionMode {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Perspective => Self::Orthographic,
            Self::Orthographic => Self::Perspective,
        }
    }
}

/// Camera controller settings.
#[derive(Resource)]
pub struct CameraSettings {
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    /// Scale movement speed by distance from focus.
    pub distance_relative_speed: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            look_sensitivity: 0.3,
            orbit_sensitivity: 0.5,
            pan_sensitivity: 0.3,
            zoom_sensitivity: 1.0,
            invert_y: false,
            distance_relative_speed: true,
        }
    }
}

/// Tracks whether the camera is actively being dragged.
#[derive(Resource, Default)]
struct CameraDragState {
    dragging: bool,
}

/// Smoothed WASD velocity for the editor camera. Each frame the controller
/// computes a target velocity from held keys and lerps the current velocity
/// toward it, so motion eases in when keys are pressed and eases out for a
/// few frames after they're released. Stored separately from `OrbitCameraState`
/// because it's transient per-session state, not something to persist in
/// scene RON.
#[derive(Resource, Default)]
struct CameraVelocityState {
    velocity: Vec3,
}

/// When `true`, zoom and pan preserve `orbit.focus` (the pivot) so orbit
/// rotations stay centered on whatever was focused. Zoom becomes a dolly
/// (changes `distance`), pan is suppressed. Engaged automatically by Focus
/// Selected (F), Frame All (A), and Camera to Cursor (End); toggle with L.
#[derive(Resource, Default)]
pub struct PivotLock(pub bool);

/// The editor 3D cursor — defined in the `renzora` contract crate so every
/// spawn path (viewport shapes dropdown, hierarchy "Add Entity", shape
/// library panel) reads the same resource. Placement lives here in
/// `renzora_camera`; see `place_3d_cursor` (Shift+RMB) and
/// `render_3d_cursor` (Gizmos::sphere).
pub use renzora::core::ThreeDCursor;

#[derive(Default)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CameraPlugin");
        app.register_type::<OrbitCameraState>()
            .init_resource::<OrbitCameraState>()
            .init_resource::<CameraSettings>()
            .init_resource::<CameraDragState>()
            .init_resource::<CameraVelocityState>()
            .init_resource::<PivotLock>()
            .init_resource::<OrbitMirror>()
            .init_resource::<EditorViewportFov>()
            .init_resource::<ThreeDCursor>()
            .register_type::<ThreeDCursor>()
            .add_systems(
                Update,
                toggle_pivot_lock.run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            )
            .add_systems(PostStartup, apply_initial_orbit)
            // Relocate the EditorCamera marker to the focused viewport before
            // the Update controller/gizmo systems read it (PreUpdate flushes
            // its structural changes before Update).
            .add_systems(
                PreUpdate,
                (relocate_editor_camera_marker, relocate_editor_2d_marker)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            )
            .add_systems(
                Update,
                (
                    // Resolve the editor viewport FOV from the active scene
                    // camera before the projection writers consume it.
                    resolve_editor_viewport_fov,
                    // In "change camera to selected" mode, snap the editor view
                    // to a scene camera the frame it's selected.
                    goto_selected_camera,
                    // Load the focused slot's angle into the singleton orbit…
                    mirror_focused_orbit_in,
                    // …then honor any per-viewport view-angle snap (each viewport's
                    // own dropdown), routing the focused slot through the live orbit
                    // and the others straight to their stored angle.
                    apply_per_slot_view_angle,
                    sync_viewport_settings,
                    handle_view_angle_keys,
                    // Frame the selected entity (Blender's NumpadPeriod). Reads
                    // world-space AABB and fits the camera distance to the
                    // bounds. Runs after `handle_view_angle_keys` and before
                    // `frame_all` so it composes correctly with other camera-
                    // view actions in the same frame.
                    frame_selected,
                    frame_all,
                    handle_camera_view_request,
                    camera_to_cursor,
                    // Place 3D cursor runs BEFORE camera_controller so the cursor
                    // resource is updated before any UI code in the same frame
                    // reads it. The cursor placement only fires on Shift+RMB
                    // just-pressed, so it does not interfere with plain RMB.
                    place_3d_cursor,
                    // Render the 3D cursor gizmo AFTER it's been updated. Runs
                    // after `place_3d_cursor` and before `camera_controller`
                    // so the visual reflects the same frame's placement.
                    render_3d_cursor,
                    camera_controller,
                    apply_nav_overlay,
                    update_camera_projection,
                    sync_orbit_snapshot,
                    apply_orbit_on_change,
                    // …persist the edited angle back to the focused slot, then
                    // drive the other views from their own stored angles.
                    mirror_focused_orbit_out,
                    apply_secondary_viewport_cameras,
                    // Last: in play mode, point the focused editor camera at the
                    // game camera so the viewport shows the running game.
                    drive_editor_camera_in_play,
                )
                    .chain()
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            );
    }
}

/// In play mode, drive the focused **editor** camera to the active game camera's
/// pose + projection.
///
/// Edit and play share one camera: the viewport already renders the editor
/// camera, so moving that camera onto the game camera's view makes the viewport
/// show the running game — with the editor's exact, proven render pipeline (live
/// atmosphere, IBL, post-process, deferred). There's no second camera, no render-
/// target swap, and no per-toggle component churn, so none of the bind-group /
/// pipelined-render crashes can happen. On Stop, the regular camera systems resume
/// and restore the editor pose from the (untouched) orbit state — it snaps back to
/// where you were editing.
#[allow(clippy::type_complexity)]
fn drive_editor_camera_in_play(
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    scene_cameras: Query<
        (&GlobalTransform, &Projection),
        (
            With<renzora::core::SceneCamera>,
            Without<EditorCamera>,
            Without<renzora::core::EditorCamera2d>,
        ),
    >,
    cameras_2d_kind: Query<(), With<bevy::camera::Camera2d>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut editor_cam: Query<
        (&mut Transform, &mut Projection),
        (With<EditorCamera>, Without<renzora::core::EditorCamera2d>),
    >,
    mut editor_cam_2d: Query<
        (&Camera, &mut Transform, &mut Projection),
        (With<renzora::core::EditorCamera2d>, Without<EditorCamera>),
    >,
    mut saved_2d_pose: Local<Option<(Transform, Projection)>>,
) {
    let playing = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());
    let game_cam = play_mode.as_ref().and_then(|pm| pm.active_game_camera);
    let game_is_2d = game_cam.is_some_and(|e| cameras_2d_kind.get(e).is_ok());

    // The 2D editor camera's pan/zoom state lives IN its transform/projection
    // (delta-based controller, no authoritative orbit resource like the 3D
    // camera has) — so driving it during play must snapshot the editor pose
    // and put it back on Stop, or play would trash where the user was editing.
    if !(playing && game_is_2d) {
        if let Some((t, p)) = saved_2d_pose.take() {
            if let Ok((_, mut tr, mut pr)) = editor_cam_2d.single_mut() {
                *tr = t;
                *pr = p;
            }
        }
    }
    if !playing {
        return;
    }
    let Some(game_cam) = game_cam else {
        return;
    };
    let Ok((gt, src_proj)) = scene_cameras.get(game_cam) else {
        return;
    };

    if game_is_2d {
        // 2D game camera: drive the editor 2D camera (Camera2d) — NOT the 3D
        // editor camera. Transplanting an orthographic projection onto the
        // Camera3d fed the sprites through the wrong pipeline and the panel
        // kept showing the editor's own pan/zoom, so sprites never lined up
        // with the camera-boundary rect during play.
        let Ok((cam, mut tr, mut pr)) = editor_cam_2d.single_mut() else {
            return;
        };
        if saved_2d_pose.is_none() {
            *saved_2d_pose = Some((*tr, pr.clone()));
        }

        // The runtime renders the game camera into a project-resolution
        // (W×H) target, so its visible world rect is W×H×ortho-scale from
        // its top-left translation (viewport_origin is (0,1) by our Godot
        // convention). The panel image has a different size/aspect, so fit
        // that rect: scale to CONTAIN it (extra world may peek beyond one
        // axis instead of letterbox bars) and center it in the panel.
        let game_scale = match src_proj {
            Projection::Orthographic(o) => o.scale,
            _ => 1.0,
        };
        let (w, h) = project
            .map(|p| {
                (
                    p.config.viewport.width.max(1) as f32,
                    p.config.viewport.height.max(1) as f32,
                )
            })
            .unwrap_or((1920.0, 1080.0));
        let game_extent = Vec2::new(w, h) * game_scale;
        let img = cam.logical_target_size().unwrap_or(game_extent);
        let fit = (game_extent.x / img.x.max(1.0)).max(game_extent.y / img.y.max(1.0));

        if let Projection::Orthographic(ref mut o) = *pr {
            o.scale = fit;
            o.viewport_origin = Vec2::new(0.0, 1.0);
        }
        let (_, _, translation) = gt.to_scale_rotation_translation();
        let visible = img * fit;
        tr.translation.x = translation.x - (visible.x - game_extent.x) * 0.5;
        tr.translation.y = translation.y + (visible.y - game_extent.y) * 0.5;
        return;
    }

    let Ok((mut transform, mut projection)) = editor_cam.single_mut() else {
        return;
    };

    let (scale, rotation, translation) = gt.to_scale_rotation_translation();
    *transform = Transform {
        translation,
        rotation,
        scale,
    };
    // Copy the game camera's projection (FOV/near); Bevy re-derives the aspect from
    // the viewport render target. Don't let an authored short far plane (Bevy's
    // 1 km default) clip the sky / distant terrain.
    *projection = src_proj.clone();
    if let Projection::Perspective(ref mut p) = *projection {
        p.far = p.far.max(100_000.0);
    }
}

/// Set the runtime camera transform from initial orbit state.
fn apply_initial_orbit(
    orbit: Res<OrbitCameraState>,
    mut cameras: Query<(Entity, &mut Transform), With<EditorCamera>>,
) {
    for (entity, mut transform) in &mut cameras {
        let t = orbit.calculate_transform();
        renzora::core::console_log::console_info(
            "Camera",
            format!(
            "apply_initial_orbit: entity={:?} focus={:?} dist={:.2} yaw={:.3} pitch={:.3} pos={:?}",
            entity, orbit.focus, orbit.distance, orbit.yaw, orbit.pitch, t.translation
        ),
        );
        *transform = t;
    }
}

/// Sync camera state from viewport header settings.
fn sync_viewport_settings(
    mut orbit: ResMut<OrbitCameraState>,
    mut settings: ResMut<CameraSettings>,
    mut vp: ResMut<ViewportSettings>,
) {
    // Sync projection mode
    let proj = match vp.projection_mode {
        VpProjectionMode::Perspective => ProjectionMode::Perspective,
        VpProjectionMode::Orthographic => ProjectionMode::Orthographic,
    };
    orbit.projection_mode = proj;

    // Sync camera settings
    let c = &vp.camera;
    settings.move_speed = c.move_speed;
    settings.look_sensitivity = c.look_sensitivity;
    settings.orbit_sensitivity = c.orbit_sensitivity;
    settings.pan_sensitivity = c.pan_sensitivity;
    settings.zoom_sensitivity = c.zoom_sensitivity;
    settings.invert_y = c.invert_y;
    settings.distance_relative_speed = c.distance_relative_speed;

    // Apply pending view angle — guard so DerefMut only fires when we
    // actually have a command to consume. Otherwise the blind `.take()`
    // marks ViewportSettings as changed every frame, which cascades into
    // spurious saves and resource-change log spam elsewhere.
    if vp.pending_view_angle.is_some() {
        if let Some(cmd) = vp.pending_view_angle.take() {
            orbit.yaw = cmd.yaw;
            orbit.pitch = cmd.pitch;
        }
    }
}

/// Consume each viewport slot's own `pending_view_angle` (set by that viewport's
/// view-angle dropdown), so every viewport can be snapped to a different preset
/// independently. The focused slot goes through the live [`OrbitCameraState`]
/// (persisted by `mirror_focused_orbit_out`); the others are written straight to
/// their stored angle, which `apply_secondary_viewport_cameras` drives each frame.
fn apply_per_slot_view_angle(
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    mut orbit: ResMut<OrbitCameraState>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    let focused = viewports.focused.min(VIEWPORT_COUNT - 1);
    for i in 0..VIEWPORT_COUNT {
        let Some(slot) = viewports.slots.get_mut(i) else {
            continue;
        };
        if slot.pending_view_angle.is_none() {
            continue;
        }
        let Some(cmd) = slot.pending_view_angle.take() else {
            continue;
        };
        if i == focused {
            orbit.yaw = cmd.yaw;
            orbit.pitch = cmd.pitch;
        } else {
            slot.yaw = cmd.yaw;
            slot.pitch = cmd.pitch;
        }
    }
}

/// Handle view angle and projection toggle keyboard shortcuts.
fn handle_view_angle_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut orbit: ResMut<OrbitCameraState>,
    mut vp: ResMut<ViewportSettings>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    use std::f32::consts::{FRAC_PI_2, PI};

    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::ViewFront, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewBack, &keyboard) {
        orbit.yaw = PI;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewLeft, &keyboard) {
        orbit.yaw = -FRAC_PI_2;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewRight, &keyboard) {
        orbit.yaw = FRAC_PI_2;
        orbit.pitch = 0.0;
    }
    if keybindings.just_pressed(EditorAction::ViewTop, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = FRAC_PI_2;
    }
    if keybindings.just_pressed(EditorAction::ViewBottom, &keyboard) {
        orbit.yaw = 0.0;
        orbit.pitch = -FRAC_PI_2;
    }
    if keybindings.just_pressed(EditorAction::ToggleProjection, &keyboard) {
        orbit.projection_mode = orbit.projection_mode.toggle();
        // Sync back to viewport settings
        vp.projection_mode = match orbit.projection_mode {
            ProjectionMode::Perspective => VpProjectionMode::Perspective,
            ProjectionMode::Orthographic => VpProjectionMode::Orthographic,
        };
    }
    if keybindings.just_pressed(EditorAction::ResetCamera, &keyboard) {
        let def = OrbitCameraState::default();
        orbit.focus = def.focus;
        orbit.distance = def.distance;
        orbit.yaw = def.yaw;
        orbit.pitch = def.pitch;
    }
}

/// Frame the camera on the selected entity — focus on the entity's bounding-box
/// center and zoom to fit its bounds, with a small margin. Blender's equivalent
/// is `NumpadPeriod` (`.` on the numpad). Per user request this does NOT engage
/// `pivot_lock`: the previous `FocusSelected` left users unable to pan until
/// they pressed L to release the lock, which felt like a bug. This version just
/// frames the entity; the user can immediately pan/orbit/zoom afterwards.
fn frame_selected(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    selection: Res<EditorSelection>,
    transforms: Query<&Transform, Without<EditorCamera>>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: Query<&Children>,
    editor_fov: Option<Res<EditorViewportFov>>,
    mut orbit: ResMut<OrbitCameraState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if !keybindings.just_pressed(EditorAction::FrameSelected, &keyboard) {
        return;
    }

    let Some(entity) = selection.get() else {
        return;
    };

    // Walk the entity + children to gather world-space AABB extents. Mirrors
    // `compute_gizmo_pivot` in `renzora_gizmo`: 8-corner transform of each
    // Mesh3d's local AABB, then expand min/max.
    if let Some((min, max)) = collect_entity_world_aabb(entity, &aabbs, &children) {
        let center = (min + max) * 0.5;
        let max_extent = (max - min).max_element();
        // Fit `max_extent / 2` in a perspective frustum with vertical FOV fov_rad,
        // plus 1.2x margin. Default 45° matches `EditorViewportFov::default`
        // and the resolution when no scene camera has been selected yet.
        let fov_rad = editor_fov
            .map(|f| f.0)
            .unwrap_or(std::f32::consts::FRAC_PI_4);
        let margin = 1.2;
        let distance = ((max_extent * 0.5) / (fov_rad * 0.5).tan()).max(0.1) * margin;
        orbit.focus = center;
        orbit.distance = distance;
        // NB: do NOT touch `pivot_lock` — see doc comment above.
    } else if let Ok(transform) = transforms.get(entity) {
        // No Mesh3d anywhere in the subtree. Fall back to the entity's origin
        // and keep the current distance (the user can zoom manually).
        orbit.focus = transform.translation;
    }
}

/// Walk an entity + its children, transforming each `Mesh3d`'s local AABB into
/// world space via its `GlobalTransform` and unioning the corners. Mirrors
/// the structure of `compute_gizmo_pivot` / `collect_pivot_aabb` in the gizmo
/// crate; reimplemented here so `frame_selected` doesn't need a cross-crate
/// dependency on `renzora_gizmo`.
fn collect_entity_world_aabb(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    collect_entity_world_aabb_inner(entity, aabbs, children, &mut min, &mut max, &mut found);
    found.then_some((min, max))
}

fn collect_entity_world_aabb_inner(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
    min: &mut Vec3,
    max: &mut Vec3,
    found: &mut bool,
) {
    if let Ok((Some(aabb), gt)) = aabbs.get(entity) {
        *found = true;
        let center = Vec3::from(aabb.center);
        let half = Vec3::from(aabb.half_extents);
        // 8-corner transform; cheaper than building an affine matrix and runs
        // the same code Bevy uses for AABB-vs-frustum tests.
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = gt.transform_point(center + half * Vec3::new(sx, sy, sz));
                    *min = min.min(corner);
                    *max = max.max(corner);
                }
            }
        }
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_entity_world_aabb_inner(child, aabbs, children, min, max, found);
        }
    }
}

/// Toggle pivot lock on/off (keybinding L).
fn toggle_pivot_lock(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut pivot_lock: ResMut<PivotLock>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if keybindings.just_pressed(EditorAction::TogglePivotLock, &keyboard) {
        pivot_lock.0 = !pivot_lock.0;
        info!(
            "[camera] pivot lock {}",
            if pivot_lock.0 { "ON" } else { "OFF" }
        );
    }
}

/// Frame all scene entities — compute a bounding sphere over mesh entity
/// positions and set the orbit focus + distance to fit them all in view.
fn frame_all(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut orbit: ResMut<OrbitCameraState>,
    mut pivot_lock: ResMut<PivotLock>,
    meshes: Query<&GlobalTransform, (With<Mesh3d>, Without<EditorCamera>, Without<PlayModeCamera>)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if !keybindings.just_pressed(EditorAction::FrameAll, &keyboard) {
        return;
    }

    let mut count = 0u32;
    let mut centroid = Vec3::ZERO;
    for gt in &meshes {
        centroid += gt.translation();
        count += 1;
    }
    if count == 0 {
        return;
    }
    centroid /= count as f32;

    let mut max_dist = 1.0f32;
    for gt in &meshes {
        let d = gt.translation().distance(centroid);
        if d > max_dist {
            max_dist = d;
        }
    }

    orbit.focus = centroid;
    orbit.distance = (max_dist * 2.5).max(3.0);
    pivot_lock.0 = true;
}

/// Consume one-shot `CameraViewRequest`s from the View menu (Zoom In/Out,
/// Reset Zoom, Fit All) and apply them to the orbit camera.
fn handle_camera_view_request(
    mut commands: Commands,
    request: Option<Res<renzora::core::CameraViewRequest>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut orbit: ResMut<OrbitCameraState>,
    mut pivot_lock: ResMut<PivotLock>,
    meshes: Query<&GlobalTransform, (With<Mesh3d>, Without<EditorCamera>, Without<PlayModeCamera>)>,
) {
    let Some(request) = request else { return };
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        commands.remove_resource::<renzora::core::CameraViewRequest>();
        return;
    }
    match *request {
        renzora::core::CameraViewRequest::ZoomIn => {
            let delta = orbit.distance * 0.2;
            orbit.zoom(delta);
        }
        renzora::core::CameraViewRequest::ZoomOut => {
            let delta = -orbit.distance * 0.2;
            orbit.zoom(delta);
        }
        renzora::core::CameraViewRequest::ResetZoom => {
            orbit.distance = OrbitCameraState::default().distance;
        }
        renzora::core::CameraViewRequest::FrameAll => {
            let mut count = 0u32;
            let mut centroid = Vec3::ZERO;
            for gt in &meshes {
                centroid += gt.translation();
                count += 1;
            }
            if count > 0 {
                centroid /= count as f32;
                let mut max_dist = 1.0f32;
                for gt in &meshes {
                    let d = gt.translation().distance(centroid);
                    if d > max_dist {
                        max_dist = d;
                    }
                }
                orbit.focus = centroid;
                orbit.distance = (max_dist * 2.5).max(3.0);
                pivot_lock.0 = true;
            }
        }
    }
    commands.remove_resource::<renzora::core::CameraViewRequest>();
}

/// Move the camera's orbit pivot to the point under the mouse cursor (ground
/// plane intersection). Keeps the camera's world position unchanged — only
/// the pivot/distance/yaw/pitch are recomputed.
fn camera_to_cursor(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut orbit: ResMut<OrbitCameraState>,
    mut pivot_lock: ResMut<PivotLock>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if !keybindings.just_pressed(EditorAction::CameraToCursor, &keyboard) {
        return;
    }

    let Some(viewport) = viewport else { return };
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if cursor.x < vp_min.x || cursor.y < vp_min.y || cursor.x > vp_max.x || cursor.y > vp_max.y {
        return;
    }
    let Some((camera, cam_xform)) = camera_q.iter().next() else {
        return;
    };
    let viewport_pos = Vec2::new(
        (cursor.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_xform, viewport_pos) else {
        return;
    };
    let dir = ray.direction.as_vec3();
    if dir.y.abs() <= 1e-6 {
        return;
    }
    let t = -ray.origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 {
        return;
    }
    let target = ray.origin + dir * t;

    let current_cam_pos = orbit.calculate_position();
    let delta = current_cam_pos - target;
    let distance = delta.length().max(0.1);
    let yaw = delta.x.atan2(delta.z);
    let pitch = (delta.y / distance).asin().clamp(-1.5, 1.5);
    orbit.focus = target;
    orbit.distance = distance;
    orbit.yaw = yaw;
    orbit.pitch = pitch;
    pivot_lock.0 = true;
}

/// Place the editor's 3D cursor (`ThreeDCursor` resource) at the point under
/// the mouse cursor. Blender convention: Shift+RMB. The camera does NOT
/// look-drag during this gesture (the `nav_drag_mode` routing returns `None`
/// for Shift+RMB, leaving this system as the only effect of that combo).
///
/// Currently the cursor is set by intersecting the mouse ray with the y=0
/// ground plane — the same convention `camera_to_cursor` uses for the End
/// key. A future PR can extend this to ray-cast against scene meshes via
/// Bevy's picking backend; the resource write-point stays the same.
fn place_3d_cursor(
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    input_focus: Res<InputFocusState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cursor: ResMut<ThreeDCursor>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if !mouse_button.just_pressed(MouseButton::Right) {
        return;
    }
    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if !shift_held {
        return;
    }

    let Some(viewport) = viewport else { return };
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(mouse_pos) = window.cursor_position() else {
        return;
    };
    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if mouse_pos.x < vp_min.x
        || mouse_pos.y < vp_min.y
        || mouse_pos.x > vp_max.x
        || mouse_pos.y > vp_max.y
    {
        return;
    }
    let Some((camera, cam_xform)) = camera_q.iter().next() else {
        return;
    };
    let viewport_pos = Vec2::new(
        (mouse_pos.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (mouse_pos.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_xform, viewport_pos) else {
        return;
    };
    let dir = ray.direction.as_vec3();
    if dir.y.abs() <= 1e-6 {
        return;
    }
    let t = -ray.origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 {
        return;
    }
    let target = ray.origin + dir * t;
    cursor.0 = target;
}

/// Render the 3D cursor as a small wireframe sphere at the cursor's world
/// position. Without this the cursor is invisible — `place_3d_cursor` writes
/// its position to a resource, but no system draws anything for the user to
/// see. Wireframe (rather than solid) so it never occludes scene content
/// and reads as a marker, not as a real object.
fn render_3d_cursor(cursor: Res<ThreeDCursor>, mut gizmos: Gizmos) {
    // Distinct red-orange tint, close to Blender's selection red, so the
    // cursor reads as an editor overlay and doesn't blend with the scene.
    let color = Color::srgb(0.95, 0.45, 0.20);
    // Radius is small (~15 cm in default world units) so it reads as a
    // marker rather than a real object. `Isometry3d::from_translation` places
    // the sphere at the cursor's world position; Bevy 0.19's Gizmos::sphere
    // draws a column of three axis-aligned wireframe circles (X/Y/Z), which
    // is the standard "3D marker" silhouette.
    gizmos.sphere(Isometry3d::from_translation(cursor.0), 0.15, color);
}

/// What the `camera_controller` should do for this frame's drag input, given
/// the pressed mouse buttons and modifier state. Centralized so the routing
/// decision can be unit-tested without spinning up a full Bevy world.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum NavDragMode {
    /// Plain MMB or Alt+Left: rotate the camera around the pivot.
    Orbit,
    /// Shift+MMB: pan the camera + pivot along the view plane.
    /// (Shift+RMB no longer routes here — it places the 3D cursor.)
    Pan,
    /// Plain RMB: rotate yaw/pitch while preserving world-space pivot.
    Look,
    /// No navigation input pressed.
    None,
}

/// Pick the navigation mode for this frame. The order of these checks is
/// part of the spec:
///   1. Shift+MMB → Pan.
///   2. Plain RMB → Look. (Shift+RMB is reserved for the Place 3D Cursor
///      operator; the camera must not look-drag while the cursor is being
///      placed, so Shift+RMB returns `None` here.)
///   3. Plain MMB or Alt+Left → Orbit.
///   4. Otherwise → None.
fn nav_drag_mode(
    right_pressed: bool,
    middle_pressed: bool,
    alt_held: bool,
    shift_held: bool,
    left_pressed: bool,
) -> NavDragMode {
    if shift_held && middle_pressed {
        return NavDragMode::Pan;
    }
    if right_pressed && !shift_held {
        return NavDragMode::Look;
    }
    if middle_pressed || (left_pressed && alt_held) {
        return NavDragMode::Orbit;
    }
    NavDragMode::None
}

/// Apply one frame's accumulated mouse motion as a pan delta to `orbit.focus`.
/// Both Shift+RMB and Shift+MMB route through here; the difference between
/// the two is just which mouse button arrived, not the math. This keeps
/// the two bindings mathematically identical so the pan speed and direction
/// stay consistent across mouse-button preferences.
///
/// Pan direction is **natural-grab** (mouse direction = content direction):
/// mouse-left → focus moves right (camera goes left, content slides right);
/// mouse-up → focus moves down (camera goes down, content slides up).
/// This matches the spec text "lower my height" semantics. See the priority
/// knowledge rule in `AGENTS.md` for why the cross-product order is
/// `view_dir × right_dir` (not `right_dir × view_dir`) under Y-up.
fn apply_pan(
    orbit: &mut OrbitCameraState,
    mouse_motion: &mut MessageReader<MouseMotion>,
    pan_sensitivity: f32,
    slow_mult: f32,
) {
    // Slider value of 1.0 ≈ the previous hardcoded 0.03 rate. The 0.01
    // multiplier matches `look_speed` / `orbit_speed` so all three sliders
    // feel comparable on the same numerical scale.
    // The 0.004 multiplier + slider default 2.5 means the slider midpoint
    // (2.5) gives the same pan speed the old default 1.0 with the old 0.01
    // multiplier did; the new max (5.0) is twice that, and 0 stops the pan
    // entirely. Default 2.5 with `CameraSettingsState::default()` below.
    let pan_speed = pan_sensitivity * 0.004 * slow_mult * orbit.distance.max(0.5);
    let right_dir = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin()).normalize();
    let view_dir = Vec3::new(
        orbit.pitch.cos() * orbit.yaw.sin(),
        orbit.pitch.sin(),
        orbit.pitch.cos() * orbit.yaw.cos(),
    );
    // Cross-product order: `view_dir × right_dir` produces `+Y` at default view
    // (Renzora's Y-up convention). The previous `right_dir × view_dir` order
    // is the Z-up form, which produces `-Y` here and inverts the Y pan direction.
    let up_dir = view_dir.cross(right_dir).normalize();
    for ev in mouse_motion.read() {
        orbit.focus -= right_dir * ev.delta.x * pan_speed;
        orbit.focus += up_dir * ev.delta.y * pan_speed;
    }
}

fn camera_controller(
    mut orbit: ResMut<OrbitCameraState>,
    settings: Res<CameraSettings>,
    vp_settings: Option<Res<ViewportSettings>>,
    mut pivot_lock: ResMut<PivotLock>,
    mut drag: ResMut<CameraDragState>,
    mut velocity: ResMut<CameraVelocityState>,
    viewport: Option<Res<ViewportState>>,
    active_tool: Option<Res<renzora_editor_framework::ActiveTool>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera_query: Query<(&mut Transform, Has<renzora::core::LoopCutScrollConsumer>), With<EditorCamera>>,
    mut window_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // Don't touch cursor or process input during play mode
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        mouse_motion.clear();
        scroll_events.clear();
        velocity.velocity = Vec3::ZERO;
        return;
    }

    // Only drive the 3D editor camera when the viewport is showing it. In UI
    // (and 2D) mode this system would otherwise still consume mouse input and
    // orbit/pan/zoom the 3D camera in the background — even while the pointer
    // is over UI panels.
    let view = vp_settings
        .as_ref()
        .map(|s| s.viewport_view)
        .unwrap_or(ViewportView::Three);
    if view != ViewportView::Three {
        mouse_motion.clear();
        scroll_events.clear();
        velocity.velocity = Vec3::ZERO;
        return;
    }

    let viewport_hovered = viewport.as_ref().is_none_or(|v| v.hovered);

    let Ok((mut transform, loop_cut_consuming)) = camera_query.single_mut() else {
        mouse_motion.clear();
        scroll_events.clear();
        velocity.velocity = Vec3::ZERO;
        return;
    };

    let right_pressed = mouse_button.pressed(MouseButton::Right);
    let middle_pressed = mouse_button.pressed(MouseButton::Middle);
    let left_pressed = mouse_button.pressed(MouseButton::Left);
    let right_just_pressed = mouse_button.just_pressed(MouseButton::Right);
    let middle_just_pressed = mouse_button.just_pressed(MouseButton::Middle);
    let right_just_released = mouse_button.just_released(MouseButton::Right);
    let middle_just_released = mouse_button.just_released(MouseButton::Middle);
    let alt_held = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let ctrl_held =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    let invert_y = if settings.invert_y { -1.0f32 } else { 1.0 };
    let slow_mult = if ctrl_held { 0.25 } else { 1.0 };
    let distance_mult = if settings.distance_relative_speed {
        (orbit.distance / 10.0).max(0.1)
    } else {
        1.0
    };

    let look_speed = settings.look_sensitivity * 0.01 * slow_mult;
    let orbit_speed = settings.orbit_sensitivity * 0.01 * slow_mult;
    let zoom_speed = settings.zoom_sensitivity * slow_mult * distance_mult;
    let move_speed = settings.move_speed * slow_mult * distance_mult;
    let delta = time.delta_secs();

    // --- WASD smoothed velocity ---
    // Compute target velocity from held WASD/QE while right-dragging, then
    // lerp the current velocity toward it. Runs every frame so motion eases
    // out for a few frames after release rather than stopping instantly.
    //
    // In Edit mode we surrender E/Q to mesh-edit (E = extrude). WASD still
    // flies the camera; users wanting vertical nav can scroll-dolly or
    // middle-drag-pan.
    let edit_mode_active = vp_settings
        .as_ref()
        .map(|s| s.viewport_mode == ViewportMode::Edit)
        .unwrap_or(false);
    let mut target_velocity = Vec3::ZERO;
    if right_pressed && drag.dragging {
        let forward = Vec3::new(
            orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.pitch.sin(),
            orbit.pitch.cos() * orbit.yaw.cos(),
        )
        .normalize();
        let right_dir = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin()).normalize();

        let mut move_delta = Vec3::ZERO;
        if keyboard.pressed(KeyCode::KeyW) {
            move_delta -= forward;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            move_delta += forward;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            move_delta -= right_dir;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            move_delta += right_dir;
        }
        // Q/E climb and descend at their own, ground-relative pace: near the
        // floor a full-speed vertical stride overshoots straight through it or
        // rockets away from what you were lining up, and the horizontal speed's
        // `distance_mult` doesn't help — that scales with the orbit distance,
        // not with how much room is left below you. Ease from a quarter speed at
        // ground level up to full by `VERTICAL_FULL_SPEED_HEIGHT`, and treat
        // "ground" as y=0 (the editor grid's plane, and where scenes are built).
        let mut vertical = 0.0f32;
        if !edit_mode_active {
            if keyboard.pressed(KeyCode::KeyE) {
                vertical += 1.0;
            }
            if keyboard.pressed(KeyCode::KeyQ) {
                vertical -= 1.0;
            }
        }
        if move_delta.length_squared() > 0.0 {
            target_velocity = move_delta.normalize() * move_speed;
        }
        if vertical != 0.0 {
            const VERTICAL_FULL_SPEED_HEIGHT: f32 = 20.0;
            const VERTICAL_MIN_MULT: f32 = 0.25;
            let height = transform.translation.y.abs();
            let t = (height / VERTICAL_FULL_SPEED_HEIGHT).clamp(0.0, 1.0);
            let mult = VERTICAL_MIN_MULT + (1.0 - VERTICAL_MIN_MULT) * t;
            target_velocity.y += vertical * move_speed * mult;
        }
    }
    // Frame-rate independent exponential smoothing — stiffness ~14 gives
    // ~0.2s ease-in/out, subtle but noticeable.
    let smooth = (1.0 - (-14.0 * delta).exp()).clamp(0.0, 1.0);
    velocity.velocity = velocity.velocity.lerp(target_velocity, smooth);
    if velocity.velocity.length_squared() > 1e-8 {
        orbit.focus += velocity.velocity * delta;
        // WASD fly breaks pivot lock — only when actively pressing keys, not
        // during the trailing decay (otherwise pivot lock stays off forever
        // after a single tap).
        if target_velocity.length_squared() > 0.0 && pivot_lock.0 {
            pivot_lock.0 = false;
        }
    } else {
        velocity.velocity = Vec3::ZERO;
    }

    // --- Cursor lock/unlock ---
    // Only start the drag if the click originated inside the viewport.
    if (right_just_pressed || middle_just_pressed) && viewport_hovered {
        drag.dragging = true;
        mouse_motion.clear();
    }
    if right_just_released || middle_just_released {
        drag.dragging = false;
    }

    // Enforce the hidden/confined cursor every frame *while* dragging, not just
    // on the press edge — so a transient reset from another system can't re-show
    // it mid-drag. `Confined`, NOT `Locked`: winit doesn't support `Locked` on
    // Windows, and requesting the unsupported mode leaves the cursor visible
    // (the crosshair stayed on screen during a right-drag orbit). `Confined` is
    // the Windows-supported mode the markup cursor code already uses; combined
    // with `visible = false` it hides the pointer for the look-drag.
    if let Ok(mut cursor) = window_query.single_mut() {
        if drag.dragging {
            if cursor.visible {
                cursor.visible = false;
            }
            if cursor.grab_mode != CursorGrabMode::Confined {
                cursor.grab_mode = CursorGrabMode::Confined;
            }
        } else if !cursor.visible {
            cursor.visible = true;
            cursor.grab_mode = CursorGrabMode::None;
        }
    }
    // On the press frame, skip the rest so the first accumulated motion delta
    // doesn't jerk the camera.
    if (right_just_pressed || middle_just_pressed) && drag.dragging {
        return;
    }

    // --- Scroll wheel: dolly zoom (only when hovering viewport) ---
    if !viewport_hovered && !drag.dragging {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Skip scroll zoom when terrain/foliage tool is active — scroll controls brush radius instead.
    // Same skip while Edit-mode loop-cut is previewing: the modal consumes
    // the wheel to set the cut count and the camera must NOT also dolly.
    // Detected via the `LoopCutScrollConsumer` marker that `loop_cut_modal`
    // attaches to the editor camera while its preview is armed.
    let tool_active = active_tool
        .as_ref()
        .is_some_and(|t| t.is_terrain_or_foliage())
        || loop_cut_consuming;

    let mut scroll_changed = false;
    if !tool_active {
        for ev in scroll_events.read() {
            if pivot_lock.0 {
                // Pivot locked: dolly toward/away from the pivot by sliding
                // `focus` along the view forward axis. The orbit distance
                // is preserved, so the camera position moves with focus.
                let forward = Vec3::new(
                    orbit.pitch.cos() * orbit.yaw.sin(),
                    orbit.pitch.sin(),
                    orbit.pitch.cos() * orbit.yaw.cos(),
                );
                orbit.focus -= forward * ev.y * zoom_speed;
            } else {
                // Default: distance-based zoom (Blender behavior). Pivot
                // stays anchored at `orbit.focus`; only `orbit.distance`
                // changes. This is the user-visible "wheel zoom" feel.
                orbit.distance = (orbit.distance - ev.y * zoom_speed).max(0.1);
            }
            scroll_changed = true;
        }
    } else {
        scroll_events.clear();
    }

    if scroll_changed && !drag.dragging {
        let t = orbit.calculate_transform();
        *transform = t;
        mouse_motion.clear();
        return;
    }

    if !drag.dragging {
        mouse_motion.clear();
        return;
    }

    // === Drag routing: pick the mode from pressed buttons + modifiers ===
    // WASD fly is handled above via the smoothed-velocity block so motion
    // eases in/out independently of the look/pan state machine.
    let drag_mode = nav_drag_mode(
        right_pressed,
        middle_pressed,
        alt_held,
        shift_held,
        left_pressed,
    );
    match drag_mode {
        NavDragMode::Pan => {
            // Pan is suppressed when pivot is locked so the orbit stays
            // centered (mirrors the original Shift+Right guard).
            if pivot_lock.0 {
                mouse_motion.clear();
            } else {
                apply_pan(
                    &mut orbit,
                    &mut mouse_motion,
                    settings.pan_sensitivity,
                    slow_mult,
                );
            }
        }
        NavDragMode::Look => {
            // Mouse look (pivot-preserved): rotate yaw/pitch, then recompute
            // the pivot so the camera world position doesn't translate.
            let cam_pos = orbit.calculate_position();
            for ev in mouse_motion.read() {
                orbit.yaw -= ev.delta.x * look_speed;
                orbit.pitch += ev.delta.y * look_speed * invert_y;
                orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
            }
            let new_dir = Vec3::new(
                orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.pitch.sin(),
                orbit.pitch.cos() * orbit.yaw.cos(),
            );
            orbit.focus = cam_pos - new_dir * orbit.distance;
        }
        NavDragMode::Orbit => {
            for ev in mouse_motion.read() {
                orbit.yaw -= ev.delta.x * orbit_speed;
                orbit.pitch += ev.delta.y * orbit_speed * invert_y;
                orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
            }
        }
        NavDragMode::None => {
            mouse_motion.clear();
        }
    }

    // Apply orbit to transform
    let t = orbit.calculate_transform();
    *transform = t;
}

/// Apply pan/zoom from the viewport nav overlay buttons.
fn apply_nav_overlay(
    nav: Option<Res<NavOverlayState>>,
    settings: Res<CameraSettings>,
    pivot_lock: Res<PivotLock>,
    mut orbit: ResMut<OrbitCameraState>,
    mut camera_query: Query<&mut Transform, With<EditorCamera>>,
) {
    let Some(nav) = nav else { return };

    let pan_dx = nav
        .pan_delta_x
        .swap(0, std::sync::atomic::Ordering::Relaxed) as f32
        / 1000.0;
    let pan_dy = nav
        .pan_delta_y
        .swap(0, std::sync::atomic::Ordering::Relaxed) as f32
        / 1000.0;
    let zoom_dy = nav
        .zoom_delta_y
        .swap(0, std::sync::atomic::Ordering::Relaxed) as f32
        / 1000.0;

    let orbit_dx = nav
        .orbit_delta_x
        .swap(0, std::sync::atomic::Ordering::Relaxed) as f32
        / 1000.0;
    let orbit_dy = nav
        .orbit_delta_y
        .swap(0, std::sync::atomic::Ordering::Relaxed) as f32
        / 1000.0;

    let has_pan = pan_dx != 0.0 || pan_dy != 0.0;
    let has_zoom = zoom_dy != 0.0;
    let has_orbit = orbit_dx != 0.0 || orbit_dy != 0.0;

    if !has_pan && !has_zoom && !has_orbit {
        return;
    }

    if has_pan && !pivot_lock.0 {
        // `apply_pan`'s 0.01 multiplier means slider value of 1.0 ≈ a strong
        // pan; default 0.3 from `CameraSettings::default()` gives a gentle
        // feel that matches Look/Orbit at the same numeric slider position.
        // Mirrors `apply_pan`'s 0.004 multiplier — slider midpoint 2.5 gives
        // the previous default speed, slider max 5.0 doubles it, slider 0
        // stops the pan.
        let pan_speed = settings.pan_sensitivity * 0.004 * orbit.distance.max(0.5);
        let right_dir = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin()).normalize();
        let up_dir = Vec3::new(
            -orbit.pitch.sin() * orbit.yaw.sin(),
            orbit.pitch.cos(),
            -orbit.pitch.sin() * orbit.yaw.cos(),
        )
        .normalize();
        orbit.focus -= right_dir * pan_dx * pan_speed;
        orbit.focus += up_dir * pan_dy * pan_speed;
    }

    if has_zoom {
        let zoom_speed = 0.02 * orbit.distance.max(0.5);
        orbit.distance -= zoom_dy * zoom_speed;
        orbit.distance = orbit.distance.clamp(
            renzora::core::viewport_types::EDITOR_ZOOM_MIN,
            renzora::core::viewport_types::EDITOR_ZOOM_MAX,
        );
    }

    if has_orbit {
        let orbit_speed = settings.orbit_sensitivity * 0.01;
        let invert_y = if settings.invert_y { -1.0 } else { 1.0 };
        orbit.yaw -= orbit_dx * orbit_speed;
        orbit.pitch += orbit_dy * orbit_speed * invert_y;
        orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
    }

    if let Ok(mut transform) = camera_query.single_mut() {
        *transform = orbit.calculate_transform();
    }
}

/// Desired editor-viewport perspective FOV (radians), mirrored from the active
/// scene camera so the editor view previews the game camera's field of view.
///
/// `apply_projection` is the **single** writer of the viewport cameras'
/// projection, so the fov is fed through it rather than via a separate
/// post-write system. A previous attempt wrote the fov in its own system that
/// ran *after* `apply_projection` had already written aspect/far — touching the
/// projection twice per frame — which jolted the atmosphere/TAA on the primary
/// viewport camera. Folding it into the one writer keeps the projection written
/// exactly once per camera per frame.
#[derive(Resource)]
struct EditorViewportFov(f32);

impl Default for EditorViewportFov {
    fn default() -> Self {
        Self(std::f32::consts::FRAC_PI_4)
    }
}

/// Mirror the active scene camera's perspective FOV into [`EditorViewportFov`]
/// (the `DefaultCamera`, else the first `SceneCamera`; falls back to the 45°
/// default when there's no scene camera). Runs before the projection writers.
fn resolve_editor_viewport_fov(
    vp_settings: Option<Res<ViewportSettings>>,
    selection: Option<Res<EditorSelection>>,
    scene_cams: Query<
        (Entity, &Projection, Has<renzora::DefaultCamera>),
        With<renzora::SceneCamera>,
    >,
    mut out: ResMut<EditorViewportFov>,
) {
    use renzora::core::viewport_types::EditorCameraSource;
    let source = vp_settings
        .as_ref()
        .map(|s| s.camera.editor_camera_source)
        .unwrap_or_default();

    let mut fov = None;
    if source == EditorCameraSource::Selected {
        if let Some(sel) = selection.as_ref().and_then(|s| s.get()) {
            if let Ok((_, Projection::Perspective(p), _)) = scene_cams.get(sel) {
                fov = Some(p.fov);
            }
        }
    }
    if fov.is_none() {
        let mut first = None;
        for (_, proj, is_default) in &scene_cams {
            if let Projection::Perspective(p) = proj {
                if is_default {
                    fov = Some(p.fov);
                    break;
                }
                if first.is_none() {
                    first = Some(p.fov);
                }
            }
        }
        fov = fov.or(first);
    }
    let fov = fov.unwrap_or(std::f32::consts::FRAC_PI_4);
    if out.0 != fov {
        out.0 = fov;
    }
}

/// In `EditorCameraSource::Selected` mode, jump the editor fly-camera to a scene
/// camera's pose the moment it's selected (one-shot per selection change).
fn goto_selected_camera(
    vp_settings: Option<Res<ViewportSettings>>,
    selection: Option<Res<EditorSelection>>,
    scene_cams: Query<&GlobalTransform, With<renzora::SceneCamera>>,
    mut orbit: ResMut<OrbitCameraState>,
    mut last: Local<Option<Entity>>,
) {
    use renzora::core::viewport_types::EditorCameraSource;
    let source = vp_settings
        .as_ref()
        .map(|s| s.camera.editor_camera_source)
        .unwrap_or_default();
    if source != EditorCameraSource::Selected {
        *last = None;
        return;
    }
    let selected = selection.as_ref().and_then(|s| s.get());
    if selected == *last {
        return;
    }
    *last = selected;
    if let Some(e) = selected {
        if let Ok(gt) = scene_cams.get(e) {
            // One line per selection (not per frame) — handy to see when the
            // editor view snaps to a scene camera.
            renzora::core::console_log::console_info(
                "CameraGoto",
                format!("snapped editor view to selected scene camera {e:?}"),
            );
            let t = gt.compute_transform();
            orbit.set_from_view(t.translation, t.rotation);
        }
    }
}

/// Apply a perspective/orthographic projection to one camera, matching the
/// editor's conventions (seamless ortho↔perspective at the orbit distance,
/// metre-scale FixedVertical ortho). Shared by the focused-camera updater and
/// the secondary-viewport sync.
fn apply_projection(
    projection: &mut Projection,
    mode: ProjectionMode,
    distance: f32,
    aspect: f32,
    fov: f32,
) {
    match mode {
        ProjectionMode::Perspective => {
            if !matches!(*projection, Projection::Perspective(_)) {
                *projection = Projection::Perspective(PerspectiveProjection {
                    fov,
                    aspect_ratio: aspect,
                    far: 100_000.0,
                    ..default()
                });
            } else if let Projection::Perspective(ref mut persp) = *projection {
                persp.aspect_ratio = aspect;
                persp.far = 100_000.0;
                persp.fov = fov;
            }
        }
        ProjectionMode::Orthographic => {
            // Match the perspective FOV at the orbit-focus distance so the
            // toggle is seamless: ortho's vertical world extent =
            // 2 * distance * tan(fov / 2). `default_3d()` ships with a
            // pixel-units scaling mode, which makes scale=1 mean "1 pixel
            // per world unit" — useless for a metre-scale 3D scene.
            // FixedVertical pins the visible world-height directly in
            // metres, independent of viewport pixel size.
            let viewport_height = 2.0 * distance * (fov * 0.5).tan();
            if !matches!(*projection, Projection::Orthographic(_)) {
                let mut ortho = OrthographicProjection::default_3d();
                ortho.scaling_mode = bevy::camera::ScalingMode::FixedVertical { viewport_height };
                ortho.scale = 1.0;
                ortho.far = 100_000.0;
                ortho.near = -100_000.0;
                *projection = Projection::Orthographic(ortho);
            } else if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scaling_mode = bevy::camera::ScalingMode::FixedVertical { viewport_height };
                ortho.scale = 1.0;
            }
        }
    }
}

/// Update the focused camera's projection based on orbit state.
fn update_camera_projection(
    orbit: Res<OrbitCameraState>,
    viewport: Option<Res<ViewportState>>,
    fov: Res<EditorViewportFov>,
    mut camera_query: Query<&mut Projection, With<EditorCamera>>,
) {
    if !orbit.is_changed() && !fov.is_changed() {
        return;
    }

    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    let aspect = viewport
        .as_ref()
        .filter(|v| v.screen_size.x > 0.0 && v.screen_size.y > 0.0)
        .map(|v| v.screen_size.x / v.screen_size.y)
        .unwrap_or(16.0 / 9.0);

    apply_projection(
        &mut projection,
        orbit.projection_mode,
        orbit.distance,
        aspect,
        fov.0,
    );
}

// ── Multi-viewport plumbing ─────────────────────────────────────────────────
//
// The editor keeps one singleton `OrbitCameraState` (and the `EditorCamera`
// marker) representing whichever viewport the user is focused on, so the whole
// existing single-camera controller / gizmo / overlay stack "just works" on the
// focused view. These systems mirror the focused slot in and out of that
// singleton and drive the other slots' cameras directly from their stored orbit.

/// Move the `EditorCamera` marker onto the focused viewport's camera (and off
/// the others) so every `With<EditorCamera>` system targets the focused view.
/// Runs in `PreUpdate` so the structural change is flushed before the `Update`
/// controller/gizmo systems read it.
fn relocate_editor_camera_marker(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    cameras: Query<(Entity, &ViewportCamera, Has<EditorCamera>)>,
    mut commands: Commands,
) {
    let focused = viewports.focused;
    for (entity, vc, has_marker) in cameras.iter() {
        let want = vc.0 == focused;
        if want && !has_marker {
            commands.entity(entity).insert(EditorCamera);
        } else if !want && has_marker {
            commands.entity(entity).remove::<EditorCamera>();
        }
    }
}

/// Move the `EditorCamera2d` marker onto the focused viewport's 2D camera (and
/// off the others) so every `With<EditorCamera2d>` system — the 2D picker, grid,
/// rulers, tile/sprite tools — targets the focused view. The 2D sibling of
/// [`relocate_editor_camera_marker`]; runs in the same `PreUpdate` step so the
/// structural change is flushed before the `Update` 2D controller/tool systems
/// read it. Each slot's 2D camera keeps rendering its own image regardless of
/// the marker (that's `is_active` + `RenderTarget`, not this) — the marker only
/// decides which one the single-camera tool stack drives.
fn relocate_editor_2d_marker(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    cameras: Query<(
        Entity,
        &renzora::core::ViewportCamera2d,
        Has<renzora::core::EditorCamera2d>,
    )>,
    mut commands: Commands,
) {
    let focused = viewports.focused;
    for (entity, vc, has_marker) in cameras.iter() {
        let want = vc.0 == focused;
        if want && !has_marker {
            commands
                .entity(entity)
                .insert(renzora::core::EditorCamera2d);
        } else if !want && has_marker {
            commands
                .entity(entity)
                .remove::<renzora::core::EditorCamera2d>();
        }
    }
}

/// Tracks the slot currently bound to the singleton `OrbitCameraState` and the
/// last value mirrored out.
///
/// `active` is set by `mirror_in` and used by `mirror_out` so the write-back
/// always targets the *same* slot that was loaded — even if `Viewports.focused`
/// changes mid-frame (the focus resolver runs in another crate and the
/// scheduler may interleave it between `mirror_in` and `mirror_out`). Without
/// this, hovering a viewport would copy the previous view's angle into it.
///
/// `last_*` lets `mirror_in` tell an *external* write of the singleton (scene
/// load / tab switch / reset) apart from the value the mirror round-trips.
#[derive(Resource, Default)]
struct OrbitMirror {
    active: usize,
    last_active: usize,
    focus: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    initialized: bool,
}

/// Load the focused slot's orbit into the singleton `OrbitCameraState` at the
/// start of the camera update, so the controller edits the focused view.
///
/// If something outside the camera loop changed the singleton since the last
/// mirror-out (e.g. a scene/tab switch restoring a saved camera), that change
/// is adopted into the focused slot instead of being overwritten.
fn mirror_focused_orbit_in(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    mut orbit: ResMut<OrbitCameraState>,
    mut mirror: ResMut<OrbitMirror>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    let focused = viewports.focused.min(VIEWPORT_COUNT - 1);
    let externally_changed = mirror.initialized
        && mirror.last_active == focused
        && (orbit.focus != mirror.focus
            || orbit.distance != mirror.distance
            || orbit.yaw != mirror.yaw
            || orbit.pitch != mirror.pitch);
    // Lock the write-back target to the slot we're about to edit this frame.
    mirror.active = focused;
    if externally_changed {
        // Keep the external value; mirror-out will persist it to the slot.
        return;
    }
    if let Some(slot) = viewports.slots.get(focused) {
        // Only the placement fields are per-view; projection mode stays shared
        // (driven by the header), so leave `orbit.projection_mode` alone. Avoid
        // a spurious change-tick when the value already matches.
        if orbit.focus != slot.focus
            || orbit.distance != slot.distance
            || orbit.yaw != slot.yaw
            || orbit.pitch != slot.pitch
        {
            orbit.focus = slot.focus;
            orbit.distance = slot.distance;
            orbit.yaw = slot.yaw;
            orbit.pitch = slot.pitch;
        }
    }
}

/// Write the (possibly edited) singleton orbit back to the slot that
/// `mirror_in` loaded this frame, so the focused view's angle persists and a
/// mid-frame focus change can't redirect the write to the wrong slot.
fn mirror_focused_orbit_out(
    orbit: Res<OrbitCameraState>,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    mut mirror: ResMut<OrbitMirror>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    let active = mirror.active.min(VIEWPORT_COUNT - 1);
    if let Some(slot) = viewports.slots.get_mut(active) {
        slot.focus = orbit.focus;
        slot.distance = orbit.distance;
        slot.yaw = orbit.yaw;
        slot.pitch = orbit.pitch;
    }
    mirror.last_active = active;
    mirror.focus = orbit.focus;
    mirror.distance = orbit.distance;
    mirror.yaw = orbit.yaw;
    mirror.pitch = orbit.pitch;
    mirror.initialized = true;
}

/// Drive every *non-focused* viewport camera's transform + projection from its
/// stored slot orbit. The focused camera is handled by the regular controller
/// path, so it's skipped here to avoid double-writes.
fn apply_secondary_viewport_cameras(
    viewports: Res<renzora::core::viewport_types::Viewports>,
    vp_settings: Option<Res<ViewportSettings>>,
    fov: Res<EditorViewportFov>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut cameras: Query<(&ViewportCamera, &mut Transform, &mut Projection), Without<PlayModeCamera>>,
) {
    // During play the viewport camera is driven to the game camera's pose by
    // `drive_editor_camera_in_play` — don't fight it by re-applying orbit poses.
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    let focused = viewports.focused;
    let mode = match vp_settings.map(|s| s.projection_mode).unwrap_or_default() {
        VpProjectionMode::Perspective => ProjectionMode::Perspective,
        VpProjectionMode::Orthographic => ProjectionMode::Orthographic,
    };
    let _ = focused;
    for (vc, mut transform, mut projection) in cameras.iter_mut() {
        let Some(slot) = viewports.slots.get(vc.0) else {
            continue;
        };
        let orbit = OrbitCameraState {
            focus: slot.focus,
            distance: slot.distance,
            yaw: slot.yaw,
            pitch: slot.pitch,
            projection_mode: mode,
        };
        // Drive *every* viewport camera from its own slot (not just the
        // non-focused ones). The focused slot is kept up to date by the
        // controller via `mirror_focused_orbit_out`, so the focused camera
        // still tracks live input — but no camera ever reads a shared value,
        // which makes it structurally impossible for the views to converge.
        *transform = orbit.calculate_transform();
        apply_projection(&mut projection, mode, slot.distance, slot.aspect(), fov.0);
    }
}

/// Apply orbit transform when the resource is replaced (e.g. after scene load).
fn apply_orbit_on_change(
    orbit: Res<OrbitCameraState>,
    mut cameras: Query<&mut Transform, (With<EditorCamera>, Without<PlayModeCamera>)>,
) {
    if !orbit.is_changed() {
        return;
    }
    for mut transform in &mut cameras {
        *transform = orbit.calculate_transform();
    }
}

/// Copy orbit yaw/pitch/distance into the shared snapshot so the viewport's axis
/// gizmo and height ruler can read them.
fn sync_orbit_snapshot(orbit: Res<OrbitCameraState>, mut snapshot: ResMut<CameraOrbitSnapshot>) {
    if orbit.is_changed() {
        snapshot.yaw = orbit.yaw;
        snapshot.pitch = orbit.pitch;
        snapshot.distance = orbit.distance;
    }
}

renzora::add!(CameraPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::f32::consts::FRAC_PI_4;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    // ── orbit → position ─────────────────────────────────────────────────────

    /// The defining invariant of an orbit camera: wherever it swings to, it
    /// stays exactly `distance` from the focus.
    #[test]
    fn the_camera_always_sits_its_distance_from_the_focus() {
        for yaw in [-3.0f32, -1.0, 0.0, 0.7, 2.5] {
            for pitch in [-1.5f32, -0.4, 0.0, 0.4, 1.5] {
                let orbit = OrbitCameraState {
                    focus: Vec3::new(3.0, -2.0, 8.0),
                    distance: 12.5,
                    yaw,
                    pitch,
                    ..default()
                };
                let d = orbit.calculate_position().distance(orbit.focus);
                assert!(close(d, 12.5), "yaw {yaw} pitch {pitch} gave distance {d}");
            }
        }
    }

    #[test]
    fn zero_pitch_puts_the_camera_level_with_its_focus() {
        let orbit = OrbitCameraState {
            focus: Vec3::new(0.0, 5.0, 0.0),
            distance: 3.0,
            yaw: 1.2,
            pitch: 0.0,
            ..default()
        };
        assert!(close(orbit.calculate_position().y, 5.0));
    }

    #[test]
    fn positive_pitch_lifts_the_camera_above_the_focus() {
        let base = OrbitCameraState {
            distance: 10.0,
            yaw: 0.0,
            pitch: 0.0,
            ..default()
        };
        let raised = OrbitCameraState {
            pitch: 0.9,
            ..base.clone()
        };
        let lowered = OrbitCameraState {
            pitch: -0.9,
            ..base.clone()
        };
        assert!(raised.calculate_position().y > base.calculate_position().y);
        assert!(lowered.calculate_position().y < base.calculate_position().y);
    }

    /// Yaw is the horizontal swing, so it must move the camera in XZ and leave
    /// its height alone.
    #[test]
    fn yaw_swings_the_camera_horizontally_only() {
        let a = OrbitCameraState {
            distance: 6.0,
            yaw: 0.0,
            pitch: 0.3,
            ..default()
        };
        let b = OrbitCameraState {
            yaw: 1.4,
            ..a.clone()
        };
        let (pa, pb) = (a.calculate_position(), b.calculate_position());
        assert!(close(pa.y, pb.y), "yaw changed the camera's height");
        assert!(
            pa.xz().distance(pb.xz()) > 0.1,
            "yaw did not move the camera"
        );
    }

    #[test]
    fn the_transform_looks_back_at_the_focus() {
        let orbit = OrbitCameraState {
            focus: Vec3::new(1.0, 2.0, 3.0),
            distance: 7.0,
            yaw: 0.8,
            pitch: 0.5,
            ..default()
        };
        let transform = orbit.calculate_transform();
        let to_focus = (orbit.focus - transform.translation).normalize();
        let forward = (transform.rotation * Vec3::NEG_Z).normalize();
        assert!(
            to_focus.dot(forward) > 0.999,
            "the camera is not aimed at its focus"
        );
    }

    // ── zoom and orbit clamps ────────────────────────────────────────────────

    #[test]
    fn zooming_in_shortens_the_distance_and_out_lengthens_it() {
        let mut orbit = OrbitCameraState {
            distance: 10.0,
            ..default()
        };
        orbit.zoom(3.0);
        assert!(close(orbit.distance, 7.0));
        orbit.zoom(-2.0);
        assert!(close(orbit.distance, 9.0));
    }

    /// A distance of zero puts the camera on its own focus, and `looking_at`
    /// then has no direction to build a rotation from — the view flips or goes
    /// NaN. The floor is what stops a fast scroll doing that.
    #[test]
    fn zoom_never_reaches_the_focus_point() {
        let mut orbit = OrbitCameraState {
            distance: 1.0,
            ..default()
        };
        orbit.zoom(1000.0);
        assert!(
            orbit.distance >= 0.1,
            "distance collapsed to {}",
            orbit.distance
        );
        assert!(orbit.calculate_transform().rotation.is_finite());
    }

    /// Pitch is clamped short of ±π/2. At exactly straight-up the focus→camera
    /// vector is parallel to the Y-up reference and `looking_at` degenerates.
    #[test]
    fn orbiting_cannot_pitch_past_the_poles() {
        let mut orbit = OrbitCameraState {
            pitch: 0.0,
            ..default()
        };
        orbit.orbit(0.0, 100.0);
        assert!(orbit.pitch <= 1.5);
        assert!(orbit.pitch < std::f32::consts::FRAC_PI_2);

        orbit.orbit(0.0, -100.0);
        assert!(orbit.pitch >= -1.5);
        assert!(orbit.pitch > -std::f32::consts::FRAC_PI_2);
    }

    /// Yaw deliberately is NOT clamped — it wraps forever, so dragging round and
    /// round keeps spinning instead of hitting a wall.
    #[test]
    fn yaw_accumulates_without_a_limit() {
        let mut orbit = OrbitCameraState {
            yaw: 0.0,
            ..default()
        };
        for _ in 0..10 {
            orbit.orbit(1.0, 0.0);
        }
        assert!(close(orbit.yaw, 10.0));
    }

    #[test]
    fn focusing_moves_the_pivot_and_keeps_the_distance() {
        let mut orbit = OrbitCameraState {
            distance: 5.0,
            ..default()
        };
        orbit.focus_on(Vec3::new(9.0, 1.0, -4.0));
        assert_eq!(orbit.focus, Vec3::new(9.0, 1.0, -4.0));
        assert!(close(orbit.distance, 5.0));
        assert!(close(orbit.calculate_position().distance(orbit.focus), 5.0));
    }

    // ── set_from_view: the "go to camera preset" path ────────────────────────

    /// The round trip that matters: aim the orbit at a pose the orbit itself
    /// produced, and it must land back on the same angles. This is what "go to
    /// camera preset" relies on, and a sign error here sends the editor view
    /// somewhere unrelated to the preset.
    #[test]
    fn set_from_view_round_trips_a_pose_the_orbit_produced() {
        for (yaw, pitch) in [(0.0f32, 0.0f32), (0.7, 0.4), (-2.1, -0.9), (3.0, 1.2)] {
            let source = OrbitCameraState {
                focus: Vec3::new(2.0, 3.0, -1.0),
                distance: 8.0,
                yaw,
                pitch,
                ..default()
            };
            let transform = source.calculate_transform();

            let mut restored = OrbitCameraState {
                distance: 8.0,
                ..default()
            };
            restored.set_from_view(transform.translation, transform.rotation);

            assert!(
                close(restored.pitch, pitch),
                "pitch {pitch} -> {}",
                restored.pitch
            );
            assert!(
                close(restored.yaw.sin(), yaw.sin()) && close(restored.yaw.cos(), yaw.cos()),
                "yaw {yaw} -> {}",
                restored.yaw
            );
            assert!(
                restored.focus.distance(source.focus) < 1e-3,
                "focus {:?} -> {:?}",
                source.focus,
                restored.focus
            );
        }
    }

    /// The focus is placed `distance` ahead of the camera so a following orbit
    /// or zoom pivots around what the user is looking at, not around wherever
    /// the old focus happened to be.
    #[test]
    fn set_from_view_places_the_focus_ahead_of_the_camera() {
        let mut orbit = OrbitCameraState {
            distance: 4.0,
            ..default()
        };
        let at = Vec3::new(0.0, 0.0, 10.0);
        orbit.set_from_view(at, Quat::IDENTITY); // facing -Z

        assert!(
            close(orbit.focus.z, 6.0),
            "focus landed at {:?}",
            orbit.focus
        );
        assert!(close(orbit.calculate_position().distance(orbit.focus), 4.0));
    }

    /// Roll is dropped on purpose — the orbit camera is always Y-up. A rolled
    /// input must still produce a level view rather than a tilted horizon.
    #[test]
    fn set_from_view_discards_roll() {
        let mut rolled = OrbitCameraState {
            distance: 4.0,
            ..default()
        };
        let roll = Quat::from_rotation_z(0.9);
        rolled.set_from_view(Vec3::ZERO, roll);

        let mut level = OrbitCameraState {
            distance: 4.0,
            ..default()
        };
        level.set_from_view(Vec3::ZERO, Quat::IDENTITY);

        assert!(close(rolled.yaw, level.yaw));
        assert!(close(rolled.pitch, level.pitch));
    }

    /// A zero rotation quaternion normalizes to nothing. Writing NaN angles from
    /// it would poison the orbit for the rest of the session.
    #[test]
    fn set_from_view_ignores_a_degenerate_rotation() {
        let mut orbit = OrbitCameraState {
            yaw: 0.5,
            pitch: 0.25,
            ..default()
        };
        orbit.set_from_view(Vec3::ONE, Quat::from_xyzw(0.0, 0.0, 0.0, 0.0));
        assert!(close(orbit.yaw, 0.5));
        assert!(close(orbit.pitch, 0.25));
    }

    #[test]
    fn set_from_view_clamps_a_straight_down_view_to_the_pitch_limit() {
        let mut orbit = OrbitCameraState {
            distance: 4.0,
            ..default()
        };
        orbit.set_from_view(
            Vec3::ZERO,
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        );
        assert!(orbit.pitch <= 1.5 && orbit.pitch >= -1.5);
        assert!(orbit.calculate_transform().rotation.is_finite());
    }

    // ── projection mode ──────────────────────────────────────────────────────

    #[test]
    fn toggling_the_projection_twice_returns_to_the_start() {
        assert_eq!(
            ProjectionMode::Perspective.toggle(),
            ProjectionMode::Orthographic
        );
        assert_eq!(
            ProjectionMode::Orthographic.toggle(),
            ProjectionMode::Perspective
        );
        assert_eq!(
            ProjectionMode::default().toggle().toggle(),
            ProjectionMode::default()
        );
    }

    #[test]
    fn defaults_are_a_usable_starting_view() {
        let orbit = OrbitCameraState::default();
        assert!(
            orbit.distance > 0.1,
            "the default view must not start inside its focus"
        );
        assert!(orbit.pitch.abs() <= 1.5);
        assert_eq!(orbit.projection_mode, ProjectionMode::Perspective);

        let settings = CameraSettings::default();
        assert!(settings.move_speed > 0.0);
        assert!(settings.orbit_sensitivity > 0.0);
        assert!(settings.zoom_sensitivity > 0.0);
        assert!(!settings.invert_y);
    }

    // ── viewport FOV resolution ──────────────────────────────────────────────

    fn perspective(fov: f32) -> Projection {
        Projection::Perspective(PerspectiveProjection { fov, ..default() })
    }

    fn fov_world() -> World {
        let mut world = World::new();
        world.init_resource::<EditorViewportFov>();
        world
    }

    #[test]
    fn with_no_scene_camera_the_viewport_keeps_the_default_fov() {
        let mut world = fov_world();
        world.run_system_once(resolve_editor_viewport_fov).unwrap();
        assert!(close(world.resource::<EditorViewportFov>().0, FRAC_PI_4));
    }

    #[test]
    fn a_lone_scene_cameras_fov_is_mirrored() {
        let mut world = fov_world();
        world.spawn((renzora::SceneCamera, perspective(1.1)));
        world.run_system_once(resolve_editor_viewport_fov).unwrap();
        assert!(close(world.resource::<EditorViewportFov>().0, 1.1));
    }

    /// With several cameras in a scene, the one marked default is the one the
    /// game boots into — so it is the one the editor viewport should match,
    /// regardless of spawn order.
    #[test]
    fn the_default_camera_wins_over_the_others() {
        let mut world = fov_world();
        world.spawn((renzora::SceneCamera, perspective(0.5)));
        world.spawn((
            renzora::SceneCamera,
            renzora::DefaultCamera,
            perspective(1.3),
        ));
        world.spawn((renzora::SceneCamera, perspective(0.9)));
        world.run_system_once(resolve_editor_viewport_fov).unwrap();
        assert!(close(world.resource::<EditorViewportFov>().0, 1.3));
    }

    /// An orthographic camera has no FOV to mirror; it must be skipped rather
    /// than treated as zero, which would collapse the viewport's frustum.
    #[test]
    fn orthographic_scene_cameras_are_skipped() {
        let mut world = fov_world();
        world.spawn((
            renzora::SceneCamera,
            Projection::Orthographic(OrthographicProjection::default_3d()),
        ));
        world.run_system_once(resolve_editor_viewport_fov).unwrap();
        assert!(close(world.resource::<EditorViewportFov>().0, FRAC_PI_4));
    }

    // ── navigation drag routing ──────────────────────────────────────────────
    //
    // The bug this PR fixes is that holding Shift had no effect on MMB drag —
    // the camera_controller's MMB branch ignored `shift_held` and fell
    // straight through to the orbit path. After the fix, both Shift+RMB and
    // Shift+MMB route to pan. The routing logic is centralized in
    // `nav_drag_mode` so these tests can pin the priority order without
    // spinning up a Bevy world.

    /// Plain MMB (no Shift, no Alt): orbit mode, the default
    /// "look around the pivot" Blender-style interaction.
    #[test]
    fn plain_mmb_routes_to_orbit() {
        assert_eq!(
            nav_drag_mode(false, true, false, false, false),
            NavDragMode::Orbit
        );
    }

    /// Plain RMB (no Shift): look mode (pivot-preserved, the existing
    /// "look around" behavior that keeps the camera world-position fixed
    /// by recalculating the focus each frame).
    #[test]
    fn plain_rmb_routes_to_look() {
        assert_eq!(
            nav_drag_mode(true, false, false, false, false),
            NavDragMode::Look
        );
    }

    /// Alt+Left (no Shift): orbit mode (preserved from the existing handler,
    /// which used to share the else-if branch with plain MMB).
    #[test]
    fn plain_alt_left_routes_to_orbit() {
        assert_eq!(
            nav_drag_mode(false, false, true, false, true),
            NavDragMode::Orbit
        );
    }

    /// Shift+MMB: pan mode. Without this routing the bug is that Shift has
    /// no effect on MMB drag — the camera_controller falls through to
    /// orbit (this is the user-reported bug fixed by this PR).
    #[test]
    fn shift_mmb_routes_to_pan() {
        assert_eq!(
            nav_drag_mode(false, true, false, true, false),
            NavDragMode::Pan
        );
    }

    /// Shift+RMB: returns `None`. The camera must not look-drag here — that
    /// modifier+button combination is reserved for the Place 3D Cursor
    /// operator (Blender convention; see bug #14 in known-bugs.md).
    #[test]
    fn shift_rmb_routes_to_none() {
        assert_eq!(
            nav_drag_mode(true, false, false, true, false),
            NavDragMode::None
        );
    }

    /// Shift + Alt + Left: still orbit. The Shift modifier only switches
    /// RMB and MMB to pan; the Alt+Left orbit path is unconditional because
    /// the user's intent (Alt held) was orbit to begin with. This priority
    /// matches the spec clause "Shift modifier switches the routing of
    /// RMB and MMB" without hijacking other modifier combinations.
    #[test]
    fn shift_with_alt_left_still_orbits() {
        assert_eq!(
            nav_drag_mode(false, false, true, true, true),
            NavDragMode::Orbit
        );
    }

    /// Both RMB and MMB held with Shift: pan. The right button takes
    /// priority in the helper so a physically unusual double-press becomes
    /// a deterministic pan instead of dropping into orbit.
    #[test]
    fn shift_with_both_buttons_routes_to_pan() {
        assert_eq!(
            nav_drag_mode(true, true, false, true, false),
            NavDragMode::Pan
        );
    }

    /// No button pressed: returns `None`. The camera_controller will clear
    /// the mouse_motion buffer so stale deltas cannot leak into the next
    /// frame's orbit-rotation.
    #[test]
    fn no_button_routes_to_none() {
        assert_eq!(
            nav_drag_mode(false, false, false, false, false),
            NavDragMode::None
        );
    }

    /// Shift held alone, no mouse button: still `None`. Shift is meaningful
    /// only as a modifier for a drag input, never on its own.
    #[test]
    fn shift_alone_routes_to_none() {
        assert_eq!(
            nav_drag_mode(false, false, false, true, false),
            NavDragMode::None
        );
    }
}
