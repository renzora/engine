mod file_drop;
mod shortcuts;

pub use file_drop::{
    check_mesh_instance_models, handle_asset_panel_drop, handle_file_drop,
    spawn_loaded_gltfs, spawn_mesh_instance_models, PendingGltfLoads, PendingMeshInstanceLoads,
};
pub use shortcuts::handle_selection;

use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingGltfLoads>()
            .init_resource::<PendingMeshInstanceLoads>();
    }
}
