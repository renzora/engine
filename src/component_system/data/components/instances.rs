//! Instance-related component data types
//!
//! These components reference external assets (3D models, scenes) rather than
//! defining inline data.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for mesh instance nodes - stores the path to a 3D model file
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component)]
pub struct MeshInstanceData {
    /// Path to the 3D model file (relative to assets folder)
    /// None if no model is assigned yet
    pub model_path: Option<String>,
}

/// Data component for scene instance nodes - stores the path to a scene file
/// Scene instances appear as a single collapsed node in the hierarchy.
/// The contents are only loaded/shown when the scene is "opened" for editing.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component)]
pub struct SceneInstanceData {
    /// Path to the scene file (.ron)
    pub scene_path: String,
    /// Whether the scene instance is currently "open" for editing
    /// When open, children are shown; when closed, only the instance node is visible
    #[serde(default)]
    pub is_open: bool,
}
