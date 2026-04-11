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

use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use renzora::core::InputFocusState;
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::{
    CameraOrbitSnapshot, NavOverlayState, ProjectionMode as VpProjectionMode,
    ViewportSettings, ViewportState,
};
use renzora::editor::EditorSelection;
use renzora::core::EditorCamera;

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
            distance: 10.0,
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
}

/// Camera projection mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect, serde::Serialize, serde::Deserialize)]
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

#[derive(Default)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CameraPlugin");
        app.register_type::<OrbitCameraState>()
            .init_resource::<OrbitCameraState>()
            .init_resource::<CameraSettings>()
            .init_resource::<CameraDragState>()
            .add_systems(PostStartup, apply_initial_orbit)
            .add_systems(Update, (
                sync_viewport_settings,
                handle_view_angle_keys,
                focus_selected,
                camera_controller,
                apply_nav_overlay,
                update_camera_projection,
                sync_orbit_snapshot,
                apply_orbit_on_change,
            ).chain().run_if(in_state(renzora::editor::SplashState::Editor)));
    }
}

/// Set the runtime camera transform from initial orbit state.
fn apply_initial_orbit(
    orbit: Res<OrbitCameraState>,
    mut cameras: Query<(Entity, &mut Transform), With<EditorCamera>>,
) {
    for (entity, mut transform) in &mut cameras {
        let t = orbit.calculate_transform();
        renzora::core::console_log::console_info("Camera", format!(
            "apply_initial_orbit: entity={:?} focus={:?} dist={:.2} yaw={:.3} pitch={:.3} pos={:?}",
            entity, orbit.focus, orbit.distance, orbit.yaw, orbit.pitch, t.translation
        ));
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

    // Apply pending view angle
    if let Some(cmd) = vp.pending_view_angle.take() {
        orbit.yaw = cmd.yaw;
        orbit.pitch = cmd.pitch;
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

    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) { return; }
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }

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
}

/// Focus the camera on the currently selected entity (F key).
fn focus_selected(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    selection: Res<EditorSelection>,
    mut orbit: ResMut<OrbitCameraState>,
    transforms: Query<&Transform, Without<EditorCamera>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) { return; }
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }

    if keybindings.just_pressed(EditorAction::FocusSelected, &keyboard) {
        if let Some(entity) = selection.get() {
            if let Ok(transform) = transforms.get(entity) {
                orbit.focus_on(transform.translation);
            }
        }
    }
}

fn camera_controller(
    mut orbit: ResMut<OrbitCameraState>,
    settings: Res<CameraSettings>,
    mut drag: ResMut<CameraDragState>,
    viewport: Option<Res<ViewportState>>,
    active_tool: Option<Res<renzora::editor::ActiveTool>>,
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
    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let viewport_hovered = viewport.as_ref().map_or(true, |v| v.hovered);

    let Ok(mut transform) = camera_query.single_mut() else {
        mouse_motion.clear();
        scroll_events.clear();
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
    let ctrl_held = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
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
    let tool_active = active_tool.as_ref().map_or(false, |t| t.is_terrain_or_foliage());

    let mut scroll_changed = false;
    if !tool_active {
        for ev in scroll_events.read() {
            let forward = Vec3::new(
                orbit.pitch.cos() * orbit.yaw.sin(),
                orbit.pitch.sin(),
                orbit.pitch.cos() * orbit.yaw.cos(),
            );
            orbit.focus -= forward * ev.y * zoom_speed;
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

    // === Right-click: look around + WASD fly ===
    if right_pressed {
        // WASD movement
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
        if keyboard.pressed(KeyCode::KeyE) {
            move_delta += Vec3::Y;
        }
        if keyboard.pressed(KeyCode::KeyQ) {
            move_delta -= Vec3::Y;
        }

        if move_delta.length_squared() > 0.0 {
            let speed_mult = if shift_held { 2.0 } else { 1.0 };
            orbit.focus += move_delta.normalize() * move_speed * speed_mult * delta;
        }

        // Mouse look
        let cam_pos = orbit.calculate_position();
        for ev in mouse_motion.read() {
            orbit.yaw -= ev.delta.x * look_speed;
            orbit.pitch += ev.delta.y * look_speed * invert_y;
            orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
        }
        // Keep camera in same position, recalculate focus
        let new_dir = Vec3::new(
            orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.pitch.sin(),
            orbit.pitch.cos() * orbit.yaw.cos(),
        );
        orbit.focus = cam_pos - new_dir * orbit.distance;
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
    mut orbit: ResMut<OrbitCameraState>,
    mut camera_query: Query<&mut Transform, With<EditorCamera>>,
) {
    let Some(nav) = nav else { return };

    let pan_dx = nav.pan_delta_x.swap(0, std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let pan_dy = nav.pan_delta_y.swap(0, std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let zoom_dy = nav.zoom_delta_y.swap(0, std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;

    let has_pan = pan_dx != 0.0 || pan_dy != 0.0;
    let has_zoom = zoom_dy != 0.0;

    if !has_pan && !has_zoom {
        return;
    }

    if has_pan {
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

    if let Ok(mut transform) = camera_query.single_mut() {
        *transform = orbit.calculate_transform();
    }
}

/// Update camera projection based on orbit state (perspective/orthographic).
fn update_camera_projection(
    orbit: Res<OrbitCameraState>,
    viewport: Option<Res<ViewportState>>,
    mut camera_query: Query<&mut Projection, With<EditorCamera>>,
) {
    if !orbit.is_changed() {
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

    match orbit.projection_mode {
        ProjectionMode::Perspective => {
            if !matches!(*projection, Projection::Perspective(_)) {
                *projection = Projection::Perspective(PerspectiveProjection {
                    fov: std::f32::consts::FRAC_PI_4,
                    aspect_ratio: aspect,
                    ..default()
                });
            } else if let Projection::Perspective(ref mut persp) = *projection {
                persp.aspect_ratio = aspect;
            }
        }
        ProjectionMode::Orthographic => {
            if !matches!(*projection, Projection::Orthographic(_)) {
                let mut ortho = OrthographicProjection::default_3d();
                ortho.scale = orbit.distance / 5.0;
                *projection = Projection::Orthographic(ortho);
            } else if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scale = orbit.distance / 5.0;
            }
        }
    }
}

/// Apply orbit transform when the resource is replaced (e.g. after scene load).
fn apply_orbit_on_change(
    orbit: Res<OrbitCameraState>,
    mut cameras: Query<(Entity, &mut Transform, &Camera), With<EditorCamera>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
) {
    if !orbit.is_changed() { return; }
    let is_playing = play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode());
    for (entity, mut transform, camera) in &mut cameras {
        let new_t = orbit.calculate_transform();
        renzora::core::console_log::console_info("Camera", format!(
            "apply_orbit_on_change: entity={:?} active={} playing={} pos={:?} -> {:?} focus={:?} dist={:.2} yaw={:.3} pitch={:.3}",
            entity, camera.is_active, is_playing,
            transform.translation, new_t.translation,
            orbit.focus, orbit.distance, orbit.yaw, orbit.pitch
        ));
        *transform = new_t;
    }
}

/// Copy orbit yaw/pitch into the shared snapshot so the viewport axis gizmo can read it.
fn sync_orbit_snapshot(
    orbit: Res<OrbitCameraState>,
    mut snapshot: ResMut<CameraOrbitSnapshot>,
) {
    if orbit.is_changed() {
        snapshot.yaw = orbit.yaw;
        snapshot.pitch = orbit.pitch;
    }
}
