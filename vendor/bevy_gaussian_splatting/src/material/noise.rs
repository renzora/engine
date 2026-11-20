use bevy::prelude::*;
use bevy_interleave::prelude::{Planar, PlanarHandle};
use noise::{NoiseFn, RidgedMulti, Simplex};

use crate::{PlanarGaussian3dHandle, gaussian::formats::planar_3d::PlanarGaussian3d};

#[derive(Component, Debug, Reflect)]
pub struct NoiseMaterial {
    pub scale: f32,
}

impl Default for NoiseMaterial {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

#[derive(Default)]
pub struct NoiseMaterialPlugin;

impl Plugin for NoiseMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<NoiseMaterial>();
        app.add_systems(Update, apply_noise_cpu);
    }
}

fn apply_noise_cpu(
    mut gaussian_clouds_res: ResMut<Assets<PlanarGaussian3d>>,
    selections: Query<(&PlanarGaussian3dHandle, &NoiseMaterial), Changed<NoiseMaterial>>,
) {
    for (cloud_handle, noise_material) in selections.iter() {
        let Some(mut cloud) = gaussian_clouds_res.get_mut(cloud_handle.handle()) else {
            continue;
        };

        let rigid_multi = RidgedMulti::<Simplex>::default();
        let scale = noise_material.scale as f64;

        for index in 0..cloud.len() {
            let position = cloud.position_visibility[index].position;
            let x = position[0] as f64 * scale;
            let y = position[1] as f64 * scale;
            let z = position[2] as f64 * scale;

            for (coefficient_index, coefficient) in cloud.spherical_harmonic[index]
                .coefficients
                .iter_mut()
                .enumerate()
            {
                let noise = rigid_multi.get([x, y, z, coefficient_index as f64]);
                *coefficient = noise as f32;
            }
        }
    }
}
