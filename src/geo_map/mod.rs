pub mod coordinate;
pub mod data;
pub mod mesh;
pub mod style;
pub mod systems;
pub mod tile;
pub mod tile_cache;
pub mod tile_fetcher;

pub use data::*;
pub use style::*;

use bevy::prelude::*;

use crate::core::AppState;
use crate::project::CurrentProject;
use tile_cache::GeoTileCache;

pub struct GeoMapPlugin;

impl Plugin for GeoMapPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GeoMapData>()
            .register_type::<GeoPositionData>()
            .register_type::<GeoMarkerData>()
            .add_systems(Startup, init_tile_cache)
            .add_systems(
                Update,
                (
                    systems::geo_tile_request_system,
                    systems::geo_tile_receive_system,
                    systems::geo_atlas_build_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    systems::geo_position_sync_system,
                    systems::geo_marker_sync_system,
                )
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

fn init_tile_cache(mut commands: Commands, project: Option<Res<CurrentProject>>) {
    let cache_dir = project.map(|p| p.path.join(".renzora").join("tile_cache"));
    if let Some(ref dir) = cache_dir {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut cache = GeoTileCache::new(cache_dir);
    cache.count_disk_tiles();
    commands.insert_resource(cache);
}
