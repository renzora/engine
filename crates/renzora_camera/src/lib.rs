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
#[derive(Clone, Resource, Component, Reflect, serde::Serialize, serde::Deserialize)]
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
            pan_sensitivity: 1.0,
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
                relocate_editor_camera_marker
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
                    sync_viewport_settings,
                    handle_view_angle_keys,
                    focus_selected,
                    frame_all,
                    handle_camera_view_request,
                    camera_to_cursor,
                    camera_controller,
                    apply_nav_overlay,
                    update_camera_projection,
                    sync_orbit_snapshot,
                    apply_orbit_on_change,
                    // …persist the edited angle back to the focused slot, then
                    // drive the other views from their own stored angles.
                    mirror_focused_orbit_out,
                    apply_secondary_viewport_cameras,
                )
                    .chain()
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            );
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
    if input_focus.egui_wants_keyboard {
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

/// Focus the camera on the currently selected entity (F key).
fn focus_selected(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    selection: Res<EditorSelection>,
    mut orbit: ResMut<OrbitCameraState>,
    mut pivot_lock: ResMut<PivotLock>,
    transforms: Query<&Transform, Without<EditorCamera>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::FocusSelected, &keyboard) {
        if let Some(entity) = selection.get() {
            if let Ok(transform) = transforms.get(entity) {
                orbit.focus_on(transform.translation);
                pivot_lock.0 = true;
            }
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
    if input_focus.egui_wants_keyboard {
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
    if input_focus.egui_wants_keyboard {
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
    if input_focus.egui_wants_keyboard {
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
    mut camera_query: Query<&mut Transform, With<EditorCamera>>,
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

    let Ok(mut transform) = camera_query.single_mut() else {
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
        if !edit_mode_active {
            if keyboard.pressed(KeyCode::KeyE) {
                move_delta += Vec3::Y;
            }
            if keyboard.pressed(KeyCode::KeyQ) {
                move_delta -= Vec3::Y;
            }
        }
        if move_delta.length_squared() > 0.0 {
            target_velocity = move_delta.normalize() * move_speed;
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
    // Only start drag if the click originated inside the viewport
    if (right_just_pressed || middle_just_pressed) && viewport_hovered {
        if let Ok(mut cursor) = window_query.single_mut() {
            cursor.visible = false;
            cursor.grab_mode = CursorGrabMode::Locked;
        }
        drag.dragging = true;
        mouse_motion.clear();
        return;
    }

    if right_just_released || middle_just_released {
        if let Ok(mut cursor) = window_query.single_mut() {
            cursor.visible = true;
            cursor.grab_mode = CursorGrabMode::None;
        }
        drag.dragging = false;
    }

    // --- Scroll wheel: dolly zoom (only when hovering viewport) ---
    if !viewport_hovered && !drag.dragging {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Skip scroll zoom when terrain/foliage tool is active — scroll controls brush radius instead
    let tool_active = active_tool
        .as_ref()
        .is_some_and(|t| t.is_terrain_or_foliage());

    let mut scroll_changed = false;
    if !tool_active {
        for ev in scroll_events.read() {
            if pivot_lock.0 {
                // Dolly: move the camera along the view ray by changing
                // distance, leaving `focus` anchored.
                orbit.distance = (orbit.distance - ev.y * zoom_speed).max(0.1);
            } else {
                let forward = Vec3::new(
                    orbit.pitch.cos() * orbit.yaw.sin(),
                    orbit.pitch.sin(),
                    orbit.pitch.cos() * orbit.yaw.cos(),
                );
                orbit.focus -= forward * ev.y * zoom_speed;
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

    // === Right-click: look around (or Shift+Right to pan) ===
    // WASD fly is handled above via the smoothed-velocity block so motion
    // eases in/out independently of the look/pan state machine.
    if right_pressed {
        let right_dir = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin()).normalize();
        if shift_held {
            // Shift+Right drag = pan the camera (slide focus in view plane).
            // Suppressed when pivot is locked so orbit stays centered.
            if pivot_lock.0 {
                mouse_motion.clear();
            } else {
                let pan_speed = 0.003 * orbit.distance.max(0.5);
                let view_dir = Vec3::new(
                    orbit.pitch.cos() * orbit.yaw.sin(),
                    orbit.pitch.sin(),
                    orbit.pitch.cos() * orbit.yaw.cos(),
                );
                let up_dir = right_dir.cross(view_dir).normalize();
                for ev in mouse_motion.read() {
                    orbit.focus -= right_dir * ev.delta.x * pan_speed;
                    orbit.focus += up_dir * ev.delta.y * pan_speed;
                }
            }
        } else {
            // Mouse look (pivot-preserved).
            let cam_pos = orbit.calculate_position();
            for ev in mouse_motion.read() {
                orbit.yaw -= ev.delta.x * look_speed;
                orbit.pitch += ev.delta.y * look_speed * invert_y;
                orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
            }
            // Keep camera in same position, recalculate focus.
            let new_dir = Vec3::new(
                orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.pitch.sin(),
                orbit.pitch.cos() * orbit.yaw.cos(),
            );
            orbit.focus = cam_pos - new_dir * orbit.distance;
        }
    }
    // === Middle-click or Alt+Left: orbit ===
    else if middle_pressed || (left_pressed && alt_held) {
        for ev in mouse_motion.read() {
            orbit.yaw -= ev.delta.x * orbit_speed;
            orbit.pitch += ev.delta.y * orbit_speed * invert_y;
            orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
        }
    } else {
        mouse_motion.clear();
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
        let pan_speed = 0.003 * orbit.distance.max(0.5);
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
        orbit.distance = orbit.distance.clamp(0.5, 100.0);
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

    apply_projection(&mut projection, orbit.projection_mode, orbit.distance, aspect, fov.0);
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
    mut cameras: Query<(&ViewportCamera, &mut Transform, &mut Projection), Without<PlayModeCamera>>,
) {
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

/// Copy orbit yaw/pitch into the shared snapshot so the viewport axis gizmo can read it.
fn sync_orbit_snapshot(orbit: Res<OrbitCameraState>, mut snapshot: ResMut<CameraOrbitSnapshot>) {
    if orbit.is_changed() {
        snapshot.yaw = orbit.yaw;
        snapshot.pitch = orbit.pitch;
    }
}

renzora::add!(CameraPlugin, Editor);
