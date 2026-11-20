use rand::{
    Rng, SeedableRng,
    distr::{Distribution, StandardUniform},
    rng,
    rngs::StdRng,
    seq::SliceRandom,
};
use std::marker::Copy;

use bevy::prelude::*;
use bevy_interleave::prelude::*;
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::{
    gaussian::{
        f32::{Covariance3dOpacity, PositionVisibility, Rotation, ScaleOpacity},
        interface::{CommonCloud, TestCloud},
        iter::PositionIter,
        settings::CloudSettings,
    },
    material::spherical_harmonics::{
        HALF_SH_COEFF_COUNT, SH_COEFF_COUNT, SphericalHarmonicCoefficients,
    },
};

#[derive(
    Clone,
    Debug,
    Default,
    Copy,
    PartialEq,
    Planar,
    ReflectInterleaved,
    StorageBindings,
    Reflect,
    Pod,
    Zeroable,
    Serialize,
    Deserialize,
)]
#[serde(default)]
#[repr(C)]
pub struct Gaussian3d {
    #[serde(default)]
    pub position_visibility: PositionVisibility,
    #[serde(default)]
    pub spherical_harmonic: SphericalHarmonicCoefficients,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub scale_opacity: ScaleOpacity,
}

pub type Gaussian2d = Gaussian3d; // GaussianMode::Gaussian2d /w Gaussian3d structure

// #[allow(unused_imports)]
// #[cfg(feature = "f16")]
// use crate::gaussian::f16::{
//     Covariance3dOpacityPacked128,
//     RotationScaleOpacityPacked128,
//     pack_f32s_to_u32,
// };

// #[cfg(feature = "f16")]
// #[derive(
//     Debug,
//     Default,
//     PartialEq,
//     Reflect,
//     Serialize,
//     Deserialize,
// )]
// pub struct Cloud3d {
//     pub position_visibility: Vec<PositionVisibility>,

//     pub spherical_harmonic: Vec<SphericalHarmonicCoefficients>,

//     #[cfg(not(feature = "precompute_covariance_3d"))]
//     pub rotation_scale_opacity_packed128: Vec<RotationScaleOpacityPacked128>,

//     #[cfg(feature = "precompute_covariance_3d")]
//     pub covariance_3d_opacity_packed128: Vec<Covariance3dOpacityPacked128>,
// }

impl CommonCloud for PlanarGaussian3d {
    type PackedType = Gaussian3d;

    fn visibility(&self, index: usize) -> f32 {
        self.position_visibility[index].visibility
    }

    fn visibility_mut(&mut self, index: usize) -> &mut f32 {
        &mut self.position_visibility[index].visibility
    }

    fn position_iter(&self) -> PositionIter<'_> {
        PositionIter::new(&self.position_visibility)
    }

    #[cfg(feature = "sort_rayon")]
    fn position_par_iter(&self) -> crate::gaussian::iter::PositionParIter<'_> {
        crate::gaussian::iter::PositionParIter::new(&self.position_visibility)
    }
}

impl FromIterator<Gaussian3d> for PlanarGaussian3d {
    fn from_iter<I: IntoIterator<Item = Gaussian3d>>(iter: I) -> Self {
        iter.into_iter().collect::<Vec<Gaussian3d>>().into()
    }
}

impl From<Vec<Gaussian3d>> for PlanarGaussian3d {
    fn from(packed: Vec<Gaussian3d>) -> Self {
        Self::from_interleaved(packed)
    }
}

impl Distribution<Gaussian3d> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Gaussian3d {
        Gaussian3d {
            rotation: [
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
            ]
            .into(),
            position_visibility: [
                rng.random_range(-20.0..20.0),
                rng.random_range(-20.0..20.0),
                rng.random_range(-20.0..20.0),
                1.0,
            ]
            .into(),
            scale_opacity: [
                rng.random_range(0.0..1.0),
                rng.random_range(0.0..1.0),
                rng.random_range(0.0..1.0),
                rng.random_range(0.0..0.8),
            ]
            .into(),
            spherical_harmonic: SphericalHarmonicCoefficients {
                coefficients: {
                    // #[cfg(feature = "f16")]
                    // {
                    //     let mut coefficients: [u32; HALF_SH_COEFF_COUNT] = [0; HALF_SH_COEFF_COUNT];
                    //     for coefficient in coefficients.iter_mut() {
                    //         let upper = rng.gen_range(-1.0..1.0);
                    //         let lower = rng.gen_range(-1.0..1.0);

                    //         *coefficient = pack_f32s_to_u32(upper, lower);
                    //     }
                    //     coefficients
                    // }

                    {
                        let mut coefficients = [0.0; SH_COEFF_COUNT];
                        for coefficient in coefficients.iter_mut() {
                            *coefficient = rng.random_range(-1.0..1.0);
                        }
                        coefficients
                    }
                },
            },
        }
    }
}

pub fn random_gaussians_3d(n: usize) -> PlanarGaussian3d {
    let mut rng = rng();
    let mut gaussians: Vec<Gaussian3d> = Vec::with_capacity(n);

    for _ in 0..n {
        gaussians.push(rng.random());
    }

    PlanarGaussian3d::from_interleaved(gaussians)
}

pub fn random_gaussians_3d_seeded(n: usize, seed: u64) -> PlanarGaussian3d {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut gaussians: Vec<Gaussian3d> = Vec::with_capacity(n);

    for _ in 0..n {
        gaussians.push(StandardUniform.sample(&mut rng));
    }

    PlanarGaussian3d::from_interleaved(gaussians)
}

impl TestCloud for PlanarGaussian3d {
    fn test_model() -> Self {
        let mut rng = rng();

        let origin = Gaussian3d {
            rotation: [1.0, 0.0, 0.0, 0.0].into(),
            position_visibility: [0.0, 0.0, 0.0, 1.0].into(),
            scale_opacity: [0.125, 0.125, 0.125, 0.125].into(),
            spherical_harmonic: SphericalHarmonicCoefficients {
                coefficients: {
                    // #[cfg(feature = "f16")]
                    // {
                    //     let mut coefficients = [0_u32; HALF_SH_COEFF_COUNT];

                    //     for coefficient in coefficients.iter_mut() {
                    //         let upper = rng.gen_range(-1.0..1.0);
                    //         let lower = rng.gen_range(-1.0..1.0);

                    //         *coefficient = pack_f32s_to_u32(upper, lower);
                    //     }

                    //     coefficients
                    // }

                    {
                        let mut coefficients = [0.0; SH_COEFF_COUNT];

                        for coefficient in coefficients.iter_mut() {
                            *coefficient = rng.random_range(-1.0..1.0);
                        }

                        coefficients
                    }
                },
            },
        };
        let mut gaussians: Vec<Gaussian3d> = Vec::new();

        for &x in [-0.5, 0.5].iter() {
            for &y in [-0.5, 0.5].iter() {
                for &z in [-0.5, 0.5].iter() {
                    let mut g = origin;
                    g.position_visibility = [x, y, z, 1.0].into();
                    gaussians.push(g);

                    gaussians
                        .last_mut()
                        .unwrap()
                        .spherical_harmonic
                        .coefficients
                        .shuffle(&mut rng);
                }
            }
        }

        gaussians.push(gaussians[0]);
        gaussians.into()
    }
}

// TODO: attempt iter() on the Planar trait
impl PlanarGaussian3d {
    pub fn iter(&self) -> impl Iterator<Item = Gaussian3d> + '_ {
        self.position_visibility
            .iter()
            .zip(self.spherical_harmonic.iter())
            .zip(self.rotation.iter())
            .zip(self.scale_opacity.iter())
            .map(
                |(((position_visibility, spherical_harmonic), rotation), scale_opacity)| {
                    Gaussian3d {
                        position_visibility: *position_visibility,
                        spherical_harmonic: *spherical_harmonic,

                        rotation: *rotation,
                        scale_opacity: *scale_opacity,
                    }
                },
            )
    }
}
