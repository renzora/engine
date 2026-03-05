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
use renzora_runtime::RuntimeCamera;
use renzora_viewport::ViewportState;

/// Orbit camera state for the editor viewport.
#[derive(Resource)]
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
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

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCameraState>()
            .init_resource::<CameraSettings>()
            .init_resource::<CameraDragState>()
            .add_systems(PostStartup, apply_initial_orbit)
            .add_systems(Update, camera_controller);
    }
}

/// Set the runtime camera transform from initial orbit state.
fn apply_initial_orbit(
    orbit: Res<OrbitCameraState>,
    mut cameras: Query<&mut Transform, With<RuntimeCamera>>,
) {
    for mut transform in &mut cameras {
        let t = orbit.calculate_transform();
        *transform = t;
    }
}

fn camera_controller(
    mut orbit: ResMut<OrbitCameraState>,
    settings: Res<CameraSettings>,
    mut drag: ResMut<CameraDragState>,
    viewport: Option<Res<ViewportState>>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera_query: Query<&mut Transform, With<RuntimeCamera>>,
    mut window_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
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

    let mut scroll_changed = false;
    for ev in scroll_events.read() {
        let forward = Vec3::new(
            orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.pitch.sin(),
            orbit.pitch.cos() * orbit.yaw.cos(),
        );
        orbit.focus -= forward * ev.y * zoom_speed;
        scroll_changed = true;
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
