//! Canvas root component.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker component for a UI canvas root entity.
///
/// A canvas is a top-level container that groups UI widgets.
/// Multiple canvases can exist per scene (e.g. HUD, pause menu, inventory).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiCanvas {
    /// Render order — higher values draw on top.
    pub sort_order: i32,
    /// When to show: "always", "play_only", "editor_only".
    pub visibility_mode: String,
    /// Reference resolution width for UI scaling (design-time canvas width).
    pub reference_width: f32,
    /// Reference resolution height for UI scaling (design-time canvas height).
    pub reference_height: f32,
}

impl Default for UiCanvas {
    fn default() -> Self {
        Self {
            sort_order: 0,
            visibility_mode: "always".into(),
            reference_width: 1280.0,
            reference_height: 720.0,
        }
    }
}
