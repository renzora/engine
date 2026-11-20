#![allow(dead_code)] // ShaderType derives emit unused check helpers
use std::marker::Copy;

use half::f16;

use bevy::{prelude::*, render::render_resource::ShaderType};
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use crate::gaussian::{
    f32::{Covariance3dOpacity, Rotation, ScaleOpacity},
    formats::{planar_3d::Gaussian3d, planar_4d::Gaussian4d},
};

#[allow(dead_code)]
#[derive(
    Clone,
    Debug,
    Default,
    Copy,
    PartialEq,
    Reflect,
    ShaderType,
    Pod,
    Zeroable,
    Serialize,
    Deserialize,
)]
#[repr(C)]
pub struct RotationScaleOpacityPacked128 {
    #[reflect(ignore)]
    pub rotation: [u32; 2],
    #[reflect(ignore)]
    pub scale_opacity: [u32; 2],
}

impl RotationScaleOpacityPacked128 {
    pub fn from_gaussian(gaussian: &Gaussian3d) -> Self {
        Self {
            rotation: [
                pack_f32s_to_u32(gaussian.rotation.rotation[0], gaussian.rotation.rotation[1]),
                pack_f32s_to_u32(gaussian.rotation.rotation[2], gaussian.rotation.rotation[3]),
            ],
            scale_opacity: [
                pack_f32s_to_u32(
                    gaussian.scale_opacity.scale[0],
                    gaussian.scale_opacity.scale[1],
                ),
                pack_f32s_to_u32(
                    gaussian.scale_opacity.scale[2],
                    gaussian.scale_opacity.opacity,
                ),
            ],
        }
    }

    pub fn rotation(&self) -> Rotation {
        let (u0, l0) = unpack_u32_to_f32s(self.rotation[0]);
        let (u1, l1) = unpack_u32_to_f32s(self.rotation[1]);

        Rotation {
            rotation: [u0, l0, u1, l1],
        }
    }

    pub fn scale_opacity(&self) -> ScaleOpacity {
        let (u0, l0) = unpack_u32_to_f32s(self.scale_opacity[0]);
        let (u1, l1) = unpack_u32_to_f32s(self.scale_opacity[1]);

        ScaleOpacity {
            scale: [u0, l0, u1],
            opacity: l1,
        }
    }
}

impl From<[f32; 8]> for RotationScaleOpacityPacked128 {
    fn from(rotation_scale_opacity: [f32; 8]) -> Self {
        Self {
            rotation: [
                pack_f32s_to_u32(rotation_scale_opacity[0], rotation_scale_opacity[1]),
                pack_f32s_to_u32(rotation_scale_opacity[2], rotation_scale_opacity[3]),
            ],
            scale_opacity: [
                pack_f32s_to_u32(rotation_scale_opacity[4], rotation_scale_opacity[5]),
                pack_f32s_to_u32(rotation_scale_opacity[6], rotation_scale_opacity[7]),
            ],
        }
    }
}

impl From<[f16; 8]> for RotationScaleOpacityPacked128 {
    fn from(rotation_scale_opacity: [f16; 8]) -> Self {
        Self {
            rotation: [
                pack_f16s_to_u32(rotation_scale_opacity[0], rotation_scale_opacity[1]),
                pack_f16s_to_u32(rotation_scale_opacity[2], rotation_scale_opacity[3]),
            ],
            scale_opacity: [
                pack_f16s_to_u32(rotation_scale_opacity[4], rotation_scale_opacity[5]),
                pack_f16s_to_u32(rotation_scale_opacity[6], rotation_scale_opacity[7]),
            ],
        }
    }
}

impl From<[u32; 4]> for RotationScaleOpacityPacked128 {
    fn from(rotation_scale_opacity: [u32; 4]) -> Self {
        Self {
            rotation: [rotation_scale_opacity[0], rotation_scale_opacity[1]],
            scale_opacity: [rotation_scale_opacity[2], rotation_scale_opacity[3]],
        }
    }
}

#[allow(dead_code)]
#[derive(
    Clone,
    Debug,
    Default,
    Copy,
    PartialEq,
    Reflect,
    ShaderType,
    Pod,
    Zeroable,
    Serialize,
    Deserialize,
)]
#[repr(C)]
pub struct Covariance3dOpacityPacked128 {
    #[reflect(ignore)]
    pub cov3d: [u32; 3],
    pub opacity: u32,
}

impl Covariance3dOpacityPacked128 {
    pub fn from_gaussian(gaussian: &Gaussian3d) -> Self {
        let cov3d: Covariance3dOpacity = gaussian.into();
        let cov3d = cov3d.cov3d;

        let opacity = gaussian.scale_opacity.opacity;

        Self {
            cov3d: [
                pack_f32s_to_u32(cov3d[0], cov3d[1]),
                pack_f32s_to_u32(cov3d[2], cov3d[3]),
                pack_f32s_to_u32(cov3d[4], cov3d[5]),
            ],
            opacity: pack_f32s_to_u32(opacity, opacity), // TODO: benefit from 32-bit opacity
        }
    }

    pub fn covariance_3d_opacity(&self) -> Covariance3dOpacity {
        let (c0, c1) = unpack_u32_to_f32s(self.cov3d[0]);
        let (c2, c3) = unpack_u32_to_f32s(self.cov3d[1]);
        let (c4, c5) = unpack_u32_to_f32s(self.cov3d[2]);

        let (opacity, _) = unpack_u32_to_f32s(self.opacity);

        let cov3d: [f32; 6] = [c0, c1, c2, c3, c4, c5];

        Covariance3dOpacity {
            cov3d,
            opacity,
            pad: 0.0,
        }
    }
}

impl From<[u32; 4]> for Covariance3dOpacityPacked128 {
    fn from(cov3d_opacity: [u32; 4]) -> Self {
        Self {
            cov3d: [cov3d_opacity[0], cov3d_opacity[1], cov3d_opacity[2]],
            opacity: cov3d_opacity[3],
        }
    }
}

#[allow(dead_code)]
#[derive(
    Clone,
    Debug,
    Default,
    Copy,
    PartialEq,
    Reflect,
    ShaderType,
    Pod,
    Zeroable,
    Serialize,
    Deserialize,
)]
#[repr(C)]
pub struct IsotropicRotations {
    pub rotation: [u32; 2],
    pub rotation_r: [u32; 2],
}

impl IsotropicRotations {
    pub fn from_gaussian(gaussian: &Gaussian4d) -> Self {
        let rotation = gaussian.isotropic_rotations.rotation;
        let rotation_r = gaussian.isotropic_rotations.rotation_r;

        Self {
            rotation: [
                pack_f32s_to_u32(rotation[0], rotation[1]),
                pack_f32s_to_u32(rotation[2], rotation[3]),
            ],
            rotation_r: [
                pack_f32s_to_u32(rotation_r[0], rotation_r[1]),
                pack_f32s_to_u32(rotation_r[2], rotation_r[3]),
            ],
        }
    }

    pub fn rotations(&self) -> [Rotation; 2] {
        let (u0, l0) = unpack_u32_to_f32s(self.rotation[0]);
        let (u1, l1) = unpack_u32_to_f32s(self.rotation[1]);

        let (u0_r, l0_r) = unpack_u32_to_f32s(self.rotation_r[0]);
        let (u1_r, l1_r) = unpack_u32_to_f32s(self.rotation_r[1]);

        [
            Rotation {
                rotation: [u0, l0, u1, l1],
            },
            Rotation {
                rotation: [u0_r, l0_r, u1_r, l1_r],
            },
        ]
    }
}

impl From<[u32; 4]> for IsotropicRotations {
    fn from(rotations: [u32; 4]) -> Self {
        Self {
            rotation: [rotations[0], rotations[1]],
            rotation_r: [rotations[2], rotations[3]],
        }
    }
}

pub fn pack_f32s_to_u32(upper: f32, lower: f32) -> u32 {
    pack_f16s_to_u32(f16::from_f32(upper), f16::from_f32(lower))
}

pub fn pack_f16s_to_u32(upper: f16, lower: f16) -> u32 {
    let upper_bits = (upper.to_bits() as u32) << 16;
    let lower_bits = lower.to_bits() as u32;
    upper_bits | lower_bits
}

pub fn unpack_u32_to_f16s(value: u32) -> (f16, f16) {
    let upper = f16::from_bits((value >> 16) as u16);
    let lower = f16::from_bits((value & 0xFFFF) as u16);
    (upper, lower)
}

pub fn unpack_u32_to_f32s(value: u32) -> (f32, f32) {
    let (upper, lower) = unpack_u32_to_f16s(value);
    (upper.to_f32(), lower.to_f32())
}
