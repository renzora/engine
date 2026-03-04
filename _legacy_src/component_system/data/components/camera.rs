//! Camera-related component data types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for camera nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct CameraNodeData {
    pub fov: f32,
    /// Whether this camera should be used as the default game camera at runtime
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for CameraNodeData {
    fn default() -> Self {
        Self {
            fov: 45.0,
            is_default_camera: false,
        }
    }
}

/// Data component for camera rig nodes - a third-person camera that follows a target
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct CameraRigData {
    /// Distance from the target (how far behind)
    pub distance: f32,
    /// Height offset from the target
    pub height: f32,
    /// Horizontal offset (for over-the-shoulder cameras)
    pub horizontal_offset: f32,
    /// Field of view in degrees
    pub fov: f32,
    /// How fast the camera follows (0 = instant, higher = smoother)
    pub follow_smoothing: f32,
    /// How fast the camera rotates to look at target
    pub look_smoothing: f32,
    /// Whether this is the default game camera
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for CameraRigData {
    fn default() -> Self {
        Self {
            distance: 5.0,
            height: 2.0,
            horizontal_offset: 0.0,
            fov: 60.0,
            follow_smoothing: 5.0,
            look_smoothing: 10.0,
            is_default_camera: false,
        }
    }
}

/// Data component for 2D camera nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Camera2DData {
    /// Camera zoom level (1.0 = normal, 2.0 = 2x zoom in)
    pub zoom: f32,
    /// Whether this is the default game camera
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for Camera2DData {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            is_default_camera: false,
        }
    }
}
