#![allow(incomplete_features)]
#![cfg_attr(feature = "nightly_generic_alias", feature(lazy_type_alias))]

use bevy::prelude::*;
pub use bevy_interleave::prelude::*;

pub use camera::GaussianCamera;

pub use gaussian::{
    formats::{
        planar_3d::{
            Gaussian3d, PlanarGaussian3d, PlanarGaussian3dHandle, random_gaussians_3d,
            random_gaussians_3d_seeded,
        },
        planar_4d::{
            Gaussian4d, PlanarGaussian4d, PlanarGaussian4dHandle, random_gaussians_4d,
            random_gaussians_4d_seeded,
        },
    },
    settings::{CloudSettings, GaussianMode, RadixSortDepthBits, RasterizeMode},
};

pub use io::scene::{
    GaussianKernel, GaussianPrimitiveMetadata, GaussianPrimitiveSpec, GaussianProjection,
    GaussianScene, GaussianSceneHandle, GaussianSortingMethod, SceneCamera, SceneExportCamera,
    SceneExportCloud, write_khr_gaussian_scene_glb, write_khr_gaussian_scene_gltf,
};

pub use material::spherical_harmonics::SphericalHarmonicCoefficients;

use io::IoPlugin;

pub mod camera;
pub mod gaussian;
pub mod io;
pub mod material;
pub mod math;
pub mod morph;
pub mod query;
pub mod render;
pub mod sort;
pub mod stream;
pub mod utils;

#[cfg(feature = "noise")]
pub mod noise;

pub struct GaussianSplattingPlugin;

impl Plugin for GaussianSplattingPlugin {
    fn build(&self, app: &mut App) {
        // TODO: allow hot reloading of Cloud handle through inspector UI
        app.register_type::<SphericalHarmonicCoefficients>();

        app.add_plugins(IoPlugin);

        app.add_plugins((
            camera::GaussianCameraPlugin,
            gaussian::settings::SettingsPlugin,
            gaussian::cloud::CloudPlugin::<Gaussian3d>::default(),
            gaussian::cloud::CloudPlugin::<Gaussian4d>::default(),
        ));

        // TODO: add half types
        app.add_plugins((
            PlanarStoragePlugin::<Gaussian3d>::default(),
            PlanarStoragePlugin::<Gaussian4d>::default(),
        ));

        app.add_plugins((
            render::RenderPipelinePlugin::<Gaussian3d>::default(),
            render::RenderPipelinePlugin::<Gaussian4d>::default(),
        ));

        app.add_plugins((material::MaterialPlugin, query::QueryPlugin));

        #[cfg(feature = "noise")]
        app.add_plugins(noise::NoisePlugin);
    }
}
