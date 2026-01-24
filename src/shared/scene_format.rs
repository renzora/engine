//! Shared scene format types used by both editor and runtime
//!
//! These types define the structure of .scene files and are used for
//! serialization/deserialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Scene data - extensible format with string-based node types
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SceneData {
    /// Scene name
    pub name: String,
    /// Root nodes in the scene
    #[serde(default)]
    pub root_nodes: Vec<NodeData>,
    /// Editor camera state (used by editor, ignored by runtime)
    pub editor_camera: EditorCameraData,
}

/// Editor viewport camera state
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorCameraData {
    /// Point the camera orbits around
    pub orbit_focus: [f32; 3],
    /// Distance from focus point
    pub orbit_distance: f32,
    /// Horizontal rotation angle (radians)
    pub orbit_yaw: f32,
    /// Vertical rotation angle (radians)
    pub orbit_pitch: f32,
}

impl Default for EditorCameraData {
    fn default() -> Self {
        Self {
            orbit_focus: [0.0, 0.0, 0.0],
            orbit_distance: 10.0,
            orbit_yaw: 0.0,
            orbit_pitch: 0.3,
        }
    }
}

/// Node data version 2 - extensible with type_id string and data map
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeData {
    /// Node name displayed in hierarchy
    pub name: String,
    /// Transform data
    pub transform: TransformData,
    /// Node type identifier (e.g., "mesh.cube", "light.point")
    pub node_type: String,
    /// Type-specific data stored as a JSON-like map
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
    /// Child nodes
    #[serde(default)]
    pub children: Vec<NodeData>,
    /// Whether this node is expanded in the hierarchy tree (editor-only)
    #[serde(default)]
    pub expanded: bool,
    /// Whether this node is visible in the viewport
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Whether this node is locked from selection/editing (editor-only)
    #[serde(default)]
    pub locked: bool,
}

fn default_true() -> bool {
    true
}

/// Transform data (matches Bevy's Transform)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransformData {
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // Quaternion (x, y, z, w)
    pub scale: [f32; 3],
}

impl Default for TransformData {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl From<bevy::prelude::Transform> for TransformData {
    fn from(transform: bevy::prelude::Transform) -> Self {
        Self {
            translation: [
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
            ],
            rotation: [
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            ],
            scale: [transform.scale.x, transform.scale.y, transform.scale.z],
        }
    }
}

impl From<TransformData> for bevy::prelude::Transform {
    fn from(data: TransformData) -> Self {
        Self {
            translation: bevy::prelude::Vec3::new(
                data.translation[0],
                data.translation[1],
                data.translation[2],
            ),
            rotation: bevy::prelude::Quat::from_xyzw(
                data.rotation[0],
                data.rotation[1],
                data.rotation[2],
                data.rotation[3],
            ),
            scale: bevy::prelude::Vec3::new(data.scale[0], data.scale[1], data.scale[2]),
        }
    }
}
