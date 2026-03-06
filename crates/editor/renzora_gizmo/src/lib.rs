//! Renzora Gizmo — 3D transform gizmos for the editor viewport.
//!
//! Provides translate/rotate/scale gizmos with axis picking, drag interaction,
//! and Blender-style G/R/S modal transforms.

pub mod state;
pub mod picking;
pub mod meshes;
pub mod drawing;
pub mod interaction;
pub mod modal_transform;

pub use state::{DragAxis, EditorTool, GizmoMode, GizmoState, SnapSettings, SnapTarget};
pub use modal_transform::{
    ModalTransformState, ModalTransformMode, AxisConstraint,
    modal_transform_input_system, modal_transform_keyboard_system,
    modal_transform_apply_system, modal_transform_overlay_system,
};

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

// Gizmo constants
pub const GIZMO_SIZE: f32 = 2.0;
pub const GIZMO_PICK_THRESHOLD: f32 = 0.25;
pub const GIZMO_PLANE_SIZE: f32 = 0.5;
pub const GIZMO_PLANE_OFFSET: f32 = 0.6;
pub const GIZMO_CENTER_SIZE: f32 = 0.2;

/// Render layer for editor gizmos.
/// Using layer 0 (same as scene) since GizmoMaterial uses depth_compare: Always
/// to render on top of scene geometry regardless.
pub const GIZMO_RENDER_LAYER: usize = 0;

/// Get the render layers for the gizmo overlay camera
pub fn gizmo_overlay_layers() -> RenderLayers {
    RenderLayers::layer(GIZMO_RENDER_LAYER)
}

/// Marker component for the gizmo overlay camera
#[derive(Component)]
pub struct GizmoOverlayCamera;

/// Custom gizmo config group for selection gizmos (renders on top)
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct SelectionGizmoGroup;

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GizmoState>();
        app.init_resource::<ModalTransformState>();
        app.add_plugins(MaterialPlugin::<meshes::GizmoMaterial>::default());
        if !app.is_plugin_added::<bevy::picking::mesh_picking::MeshPickingPlugin>() {
            app.add_plugins(bevy::picking::mesh_picking::MeshPickingPlugin);
        }
        app.init_gizmo_group::<SelectionGizmoGroup>();

        app.add_systems(Startup, configure_gizmo_render_layers);
        app.add_systems(PostStartup, meshes::setup_gizmo_meshes);

        use renzora_editor::SplashState;
        app.add_systems(Update, (
            interaction::gizmo_hover_system,
            interaction::gizmo_interaction_system,
            interaction::entity_pick_system,
            interaction::object_drag_system,
            meshes::update_gizmo_mesh_transforms,
            meshes::update_gizmo_materials,
            drawing::draw_selection_gizmo,
            modal_transform_input_system,
            modal_transform_keyboard_system,
            modal_transform_apply_system,
            modal_transform_overlay_system,
        ).run_if(in_state(SplashState::Editor)));
    }
}

/// Configure gizmo render layers
fn configure_gizmo_render_layers(mut config_store: ResMut<GizmoConfigStore>) {
    let (default_config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    default_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    default_config.depth_bias = -1.0;
    default_config.line.width = 2.0;

    let (selection_config, _) = config_store.config_mut::<SelectionGizmoGroup>();
    selection_config.render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);
    selection_config.depth_bias = -1.0;
    selection_config.line.width = 3.0;
}
