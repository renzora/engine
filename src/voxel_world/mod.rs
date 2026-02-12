pub mod config;

use bevy::prelude::*;
use bevy_voxel_world::prelude::*;

use crate::component_system::components::voxel_world::VoxelWorldData;
use crate::core::{AppState, ViewportCamera};
use config::RenzoraVoxelConfig;

pub struct RenzoraVoxelWorldPlugin;

impl Plugin for RenzoraVoxelWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VoxelWorldPlugin::with_config(RenzoraVoxelConfig::default()))
            .add_systems(
                Update,
                (sync_voxel_config, sync_voxel_camera).run_if(in_state(AppState::Editor)),
            );
    }
}

/// Watches for VoxelWorldData component changes and updates the RenzoraVoxelConfig resource.
/// Only one VoxelWorldData entity is supported — takes the first found.
fn sync_voxel_config(
    query: Query<Ref<VoxelWorldData>>,
    mut config: ResMut<RenzoraVoxelConfig>,
) {
    for data in query.iter() {
        if data.is_changed() {
            config.data = data.clone();
            return;
        }
    }
}

/// Automatically adds/removes VoxelWorldCamera on the editor viewport camera
/// based on whether an enabled VoxelWorldData component exists in the scene.
fn sync_voxel_camera(
    mut commands: Commands,
    voxel_data: Query<&VoxelWorldData>,
    camera: Query<(Entity, Has<VoxelWorldCamera<RenzoraVoxelConfig>>), With<ViewportCamera>>,
) {
    let needs_marker = voxel_data.iter().any(|d| d.enabled);
    let Ok((cam_entity, has_marker)) = camera.single() else {
        return;
    };

    if needs_marker && !has_marker {
        commands
            .entity(cam_entity)
            .insert(VoxelWorldCamera::<RenzoraVoxelConfig>::default());
    } else if !needs_marker && has_marker {
        commands
            .entity(cam_entity)
            .remove::<VoxelWorldCamera<RenzoraVoxelConfig>>();
    }
}
