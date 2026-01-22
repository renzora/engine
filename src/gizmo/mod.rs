mod drawing;
mod grid;
mod interaction;
pub mod picking;

pub use drawing::draw_selection_gizmo;
pub use grid::draw_grid;
pub use interaction::{gizmo_hover_system, gizmo_interaction_system, object_drag_system};

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

// Gizmo constants
pub const GIZMO_SIZE: f32 = 2.0;
pub const GIZMO_PICK_THRESHOLD: f32 = 0.25;
pub const GIZMO_PLANE_SIZE: f32 = 0.5;
pub const GIZMO_PLANE_OFFSET: f32 = 0.6;
pub const GIZMO_CENTER_SIZE: f32 = 0.2;

/// Render layer for editor gizmos (grid, selection gizmo, etc.)
/// Layer 0 is the default scene layer, layer 1 is for editor-only visuals
pub const GIZMO_RENDER_LAYER: usize = 1;

/// Get the render layers for the main editor camera (sees everything including gizmos)
pub fn editor_camera_layers() -> RenderLayers {
    RenderLayers::layer(0).with(GIZMO_RENDER_LAYER)
}

/// Get the render layers for the camera preview (scene only, no gizmos)
pub fn preview_camera_layers() -> RenderLayers {
    RenderLayers::layer(0)
}

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        // Configure gizmos to render on the gizmo layer
        app.add_systems(Startup, configure_gizmo_render_layers);
    }
}

/// Configure the gizmo system to render on our custom render layer
fn configure_gizmo_render_layers(mut config_store: ResMut<GizmoConfigStore>) {
    // Set all gizmos to render on the gizmo layer
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
}
