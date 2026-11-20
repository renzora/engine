use bevy::prelude::*;

pub mod codec;
pub mod gcloud;
pub mod loader;
pub mod scene;

#[cfg(feature = "io_ply")]
pub mod ply;

#[derive(Default)]
pub struct IoPlugin;
impl Plugin for IoPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_loader::<loader::Gaussian3dLoader>();
        app.init_asset_loader::<loader::Gaussian4dLoader>();

        app.add_plugins(scene::GaussianScenePlugin);
    }
}
