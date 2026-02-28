mod drawing;
mod grid;
mod interaction;
pub mod meshes;
pub mod modal_transform;
mod physics;
pub mod physics_viz;
pub mod picking;
pub mod state;
pub mod camera_gizmos;

// 2D viewport modules
pub mod gizmo_2d;
pub mod interaction_2d;
pub mod picking_2d;

pub use drawing::{draw_selection_gizmo, update_selection_outlines};
pub use grid::draw_grid;
pub use interaction::{gizmo_hover_system, gizmo_interaction_system, object_drag_system, terrain_chunk_selection_system};
pub use physics::{
    draw_physics_gizmos, draw_collider_edit_handles, collider_edit_selection_sync,
    collider_edit_hover_system, collider_edit_interaction_system, collider_edit_drag_system,
};
pub use state::{ColliderEditHandle, DragAxis, EditorTool, GizmoMode, GizmoState, SnapSettings, SnapTarget};
pub use modal_transform::{
    ModalTransformState, ModalTransformMode, AxisConstraint,
    modal_transform_input_system, modal_transform_keyboard_system,
    modal_transform_apply_system, modal_transform_overlay_system,
};

// 2D gizmo exports
pub use gizmo_2d::draw_selection_gizmo_2d;
pub use interaction_2d::{gizmo_2d_hover_system, gizmo_2d_interaction_system, gizmo_2d_drag_system};
pub use picking_2d::handle_2d_picking;

// Camera gizmo exports
pub use camera_gizmos::draw_camera_gizmos;
pub use physics_viz::{PhysicsVizGizmoGroup, render_physics_debug_gizmos};

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

// Gizmo constants
pub const GIZMO_SIZE: f32 = 2.0;
pub const GIZMO_PICK_THRESHOLD: f32 = 0.25;
pub const GIZMO_PLANE_SIZE: f32 = 0.5;
pub const GIZMO_PLANE_OFFSET: f32 = 0.6;
pub const GIZMO_CENTER_SIZE: f32 = 0.2;
pub const SCREEN_PICK_RADIUS: f32 = 30.0;

/// Render layer for editor gizmos (grid, selection gizmo, etc.)
/// Layer 0 is the default scene layer, layer 1 is for editor-only visuals
pub const GIZMO_RENDER_LAYER: usize = 1;

/// Get the render layers for the main editor camera (scene only — gizmos are on a separate overlay camera)
pub fn editor_camera_layers() -> RenderLayers {
    RenderLayers::layer(0)
}

/// Get the render layers for the gizmo overlay camera (gizmos only, no scene/Solari)
pub fn gizmo_overlay_layers() -> RenderLayers {
    RenderLayers::layer(GIZMO_RENDER_LAYER)
}

/// Get the render layers for the camera preview (scene only, no gizmos)
pub fn preview_camera_layers() -> RenderLayers {
    RenderLayers::layer(0)
}

/// Marker component for the gizmo overlay camera
#[derive(Component)]
pub struct GizmoOverlayCamera;

/// Custom gizmo config group for the grid (normal depth testing)
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct GridGizmoGroup;

/// Custom gizmo config group for axis lines (renders in front of grid)
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct AxisGizmoGroup;

/// Custom gizmo config group for selection gizmos (renders on top)
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct SelectionGizmoGroup;

/// Custom gizmo config group for terrain selection (uses depth testing)
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct TerrainSelectionGizmoGroup;

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        // Initialize gizmo state
        app.init_resource::<GizmoState>();
        // Register custom gizmo config groups
        app.init_gizmo_group::<GridGizmoGroup>();
        app.init_gizmo_group::<AxisGizmoGroup>();
        app.init_gizmo_group::<SelectionGizmoGroup>();
        app.init_gizmo_group::<TerrainSelectionGizmoGroup>();
        app.init_gizmo_group::<PhysicsVizGizmoGroup>();
        // Configure gizmos to render on the gizmo layer
        app.add_systems(Startup, (configure_gizmo_render_layers, meshes::setup_gizmo_meshes));
        // Update mesh-based gizmos and selection boundary depth mode
        app.add_systems(Update, (
            meshes::update_gizmo_mesh_transforms,
            meshes::update_gizmo_materials,
            update_selection_gizmo_depth,
        ).run_if(in_state(crate::core::AppState::Editor)));
    }
}

/// Update SelectionGizmoGroup render layers based on the boundary depth setting.
fn update_selection_gizmo_depth(
    settings: Res<crate::core::EditorSettings>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if !settings.is_changed() {
        return;
    }
    let (selection_config, _) = config_store.config_mut::<SelectionGizmoGroup>();
    if settings.selection_boundary_on_top {
        selection_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
        selection_config.depth_bias = -1.0;
    } else {
        // Scene layer — participates in the main camera's depth buffer
        selection_config.render_layers = RenderLayers::layer(0);
        selection_config.depth_bias = 0.0;
    }
}

/// Configure the gizmo system with separate config groups for grid and selection
fn configure_gizmo_render_layers(mut config_store: ResMut<GizmoConfigStore>) {
    // Default gizmos - render on top of scene objects
    let (default_config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    default_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    default_config.depth_bias = -1.0;
    default_config.line.width = 2.0;

    // Grid gizmos - render on scene layer (layer 0) so they're properly occluded by objects
    let (grid_config, _) = config_store.config_mut::<GridGizmoGroup>();
    grid_config.render_layers = RenderLayers::layer(0);
    grid_config.line.width = 1.0;

    // Axis gizmos - same layer as grid but with depth bias so they draw in front of it
    let (axis_config, _) = config_store.config_mut::<AxisGizmoGroup>();
    axis_config.render_layers = RenderLayers::layer(0);
    axis_config.depth_bias = -0.5;
    axis_config.line.width = 1.0;

    // Selection gizmos - render on top of everything
    let (selection_config, _) = config_store.config_mut::<SelectionGizmoGroup>();
    selection_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    selection_config.depth_bias = -1.0;
    selection_config.line.width = 3.0;

    // Terrain selection gizmos - render on top of scene objects
    let (terrain_config, _) = config_store.config_mut::<TerrainSelectionGizmoGroup>();
    terrain_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    terrain_config.depth_bias = -1.0;
    terrain_config.line.width = 3.0;

    // Physics debug visualization gizmos - render on top of scene objects
    let (physics_viz_config, _) = config_store.config_mut::<PhysicsVizGizmoGroup>();
    physics_viz_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    physics_viz_config.depth_bias = -1.0;
    physics_viz_config.line.width = 2.0;
}
