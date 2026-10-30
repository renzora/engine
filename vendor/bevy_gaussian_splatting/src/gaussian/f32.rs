#![allow(dead_code)] // ShaderType derives emit unused check helpers
use std::marker::Copy;

use bevy::{prelude::*, render::render_resource::ShaderType};
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use crate::gaussian::{
    covariance::compute_covariance_3d,
    formats::{planar_3d::Gaussian3d, planar_4d::Gaussian4d},
};

pub type Position = [f32; 3];

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
pub struct PositionTimestamp {
    pub position: Position,
    pub timestamp: f32,
}

impl From<[f32; 4]> for PositionTimestamp {
    fn from(position_timestamp: [f32; 4]) -> Self {
        Self {
            position: [
                position_timestamp[0],
                position_timestamp[1],
                position_timestamp[2],
            ],
            timestamp: position_timestamp[3],
        }
    }
}

#[allow(dead_code)]
#[derive(
    Clone, Debug, Copy, PartialEq, Reflect, ShaderType, Pod, Zeroable, Serialize, Deserialize,
)]
#[repr(C)]
pub struct PositionVisibility {
    pub position: Position,
    pub visibility: f32,
}

impl Default for PositionVisibility {
    fn default() -> Self {
        Self {
            position: Position::default(),
            visibility: 1.0,
        }
    }
}

impl From<[f32; 4]> for PositionVisibility {
    fn from(position_visibility: [f32; 4]) -> Self {
        Self {
            position: [
                position_visibility[0],
                position_visibility[1],
                position_visibility[2],
            ],
            visibility: position_visibility[3],
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
pub struct Rotation {
    pub rotation: [f32; 4],
}

impl From<[f32; 4]> for Rotation {
    fn from(rotation: [f32; 4]) -> Self {
        Self { rotation }
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
    pub rotation: [f32; 4],
    pub rotation_r: [f32; 4],
}

impl IsotropicRotations {
    pub fn from_gaussian(gaussian: &Gaussian4d) -> Self {
        let rotation = gaussian.isotropic_rotations.rotation;
        let rotation_r = gaussian.isotropic_rotations.rotation_r;

        Self {
            rotation,
            rotation_r,
        }
    }

    pub fn rotations(&self) -> [Rotation; 2] {
        [
            Rotation {
                rotation: self.rotation,
            },
            Rotation {
                rotation: self.rotation_r,
            },
        ]
    }
}

impl From<[f32; 8]> for IsotropicRotations {
    fn from(rotations: [f32; 8]) -> Self {
        Self {
            rotation: [rotations[0], rotations[1], rotations[2], rotations[3]],
            rotation_r: [rotations[4], rotations[5], rotations[6], rotations[7]],
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
pub struct ScaleOpacity {
    pub scale: [f32; 3],
    pub opacity: f32,
}

impl From<[f32; 4]> for ScaleOpacity {
    fn from(scale_opacity: [f32; 4]) -> Self {
        Self {
            scale: [scale_opacity[0], scale_opacity[1], scale_opacity[2]],
            opacity: scale_opacity[3],
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
pub struct TimestampTimescale {
    pub timestamp: f32,
    pub timescale: f32,
    pub _pad: [f32; 2],
}

impl From<[f32; 4]> for TimestampTimescale {
    fn from(timestamp_timescale: [f32; 4]) -> Self {
        Self {
            timestamp: timestamp_timescale[0],
            timescale: timestamp_timescale[1],
            _pad: [0.0, 0.0],
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
pub struct Covariance3dOpacity {
    pub cov3d: [f32; 6],
    pub opacity: f32,
    pub pad: f32,
}

impl From<&Gaussian3d> for Covariance3dOpacity {
    fn from(gaussian: &Gaussian3d) -> Self {
        let cov3d = compute_covariance_3d(
            Vec4::from_slice(gaussian.rotation.rotation.as_slice()),
            Vec3::from_slice(gaussian.scale_opacity.scale.as_slice()),
        );

        Covariance3dOpacity {
            cov3d,
            opacity: gaussian.scale_opacity.opacity,
            pad: 0.0,
        }
    }
}
