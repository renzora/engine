mod file_drop;
mod shortcuts;

pub use file_drop::{
    check_mesh_instance_models, handle_asset_panel_drop, handle_file_drop,
    handle_image_panel_drop, handle_material_panel_drop, apply_material_data,
    spawn_loaded_gltfs, spawn_mesh_instance_models, align_models_to_ground,
    handle_scene_hierarchy_drop, handle_script_hierarchy_drop, load_scene_instances, handle_pending_skybox_drop,
    start_drag_preview, update_drag_preview, cleanup_drag_preview, drag_preview_active,
    PendingGltfLoads, PendingMeshInstanceLoads, MaterialApplied, DragPreviewState,
};
pub use shortcuts::{handle_selection, handle_view_angles, handle_view_toggles, handle_play_mode};

use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingGltfLoads>()
            .init_resource::<PendingMeshInstanceLoads>()
            .init_resource::<DragPreviewState>();
    }
}
