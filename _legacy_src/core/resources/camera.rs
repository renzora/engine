#![allow(dead_code)]

use bevy::prelude::*;

/// Camera projection mode
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ProjectionMode {
    #[default]
    Perspective,
    Orthographic,
}

impl ProjectionMode {
    pub fn toggle(&self) -> Self {
        match self {
            ProjectionMode::Perspective => ProjectionMode::Orthographic,
            ProjectionMode::Orthographic => ProjectionMode::Perspective,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ProjectionMode::Perspective => "Perspective",
            ProjectionMode::Orthographic => "Orthographic",
        }
    }
}

/// Orbit camera state for the editor viewport
#[derive(Resource)]
pub struct OrbitCameraState {
    /// The point the camera orbits around
    pub focus: Vec3,
    /// Distance from the focus point
    pub distance: f32,
    /// Horizontal rotation angle (radians)
    pub yaw: f32,
    /// Vertical rotation angle (radians)
    pub pitch: f32,
    /// Camera projection mode (perspective or orthographic)
    pub projection_mode: ProjectionMode,
}

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            distance: 10.0,
            yaw: 0.3,
            pitch: 0.4,
            projection_mode: ProjectionMode::default(),
        }
    }
}

impl OrbitCameraState {
    /// Calculate the camera position based on orbit parameters
    pub fn calculate_position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.focus + Vec3::new(x, y, z)
    }

    /// Calculate the camera transform based on orbit parameters
    pub fn calculate_transform(&self) -> Transform {
        let position = self.calculate_position();
        Transform::from_translation(position).looking_at(self.focus, Vec3::Y)
    }

    /// Focus on a specific point
    pub fn focus_on(&mut self, point: Vec3) {
        self.focus = point;
    }

    /// Zoom in/out by a delta
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).max(0.1);
    }

    /// Orbit by delta angles
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.5, 1.5);
    }

    /// Create a snapshot for scene tab storage
    pub fn to_tab_state(&self) -> TabCameraState {
        TabCameraState {
            orbit_focus: self.focus,
            orbit_distance: self.distance,
            orbit_yaw: self.yaw,
            orbit_pitch: self.pitch,
            projection_mode: self.projection_mode,
        }
    }

    /// Restore from a scene tab snapshot
    pub fn from_tab_state(&mut self, state: &TabCameraState) {
        self.focus = state.orbit_focus;
        self.distance = state.orbit_distance;
        self.yaw = state.orbit_yaw;
        self.pitch = state.orbit_pitch;
        self.projection_mode = state.projection_mode;
    }
}

/// Stored camera state when switching scene tabs
#[derive(Clone, Debug)]
pub struct TabCameraState {
    pub orbit_focus: Vec3,
    pub orbit_distance: f32,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub projection_mode: ProjectionMode,
}
