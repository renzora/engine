#![allow(dead_code)] // ShaderType derives emit unused check helpers
use std::marker::Copy;

use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
    render::render_resource::ShaderType,
};
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeTuple};

// #[cfg(feature = "f16")]
// use half::f16;

use crate::math::pad_4;

const SPHERICAL_HARMONICS_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("879b9cd3-ba20-4030-a8f3-adda0a042ffe");

pub struct SphericalHarmonicCoefficientsPlugin;

impl Plugin for SphericalHarmonicCoefficientsPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            SPHERICAL_HARMONICS_SHADER_HANDLE,
            "spherical_harmonics.wgsl",
            Shader::from_wgsl
        );
    }
}

const fn num_sh_coefficients(degree: usize) -> usize {
    if degree == 0 {
        1
    } else {
        2 * degree + 1 + num_sh_coefficients(degree - 1)
    }
}

// TODO: let SH_DEGREE be a const generic parameter to SphericalHarmonicCoefficients
// Prefer the highest enabled SH degree when multiple degree features are active.
#[cfg(feature = "sh4")]
pub const SH_DEGREE: usize = 4;

#[cfg(all(not(feature = "sh4"), feature = "sh3"))]
pub const SH_DEGREE: usize = 3;

#[cfg(all(not(feature = "sh4"), not(feature = "sh3"), feature = "sh2"))]
pub const SH_DEGREE: usize = 2;

#[cfg(all(
    not(feature = "sh4"),
    not(feature = "sh3"),
    not(feature = "sh2"),
    feature = "sh1"
))]
pub const SH_DEGREE: usize = 1;

#[cfg(all(
    not(feature = "sh4"),
    not(feature = "sh3"),
    not(feature = "sh2"),
    not(feature = "sh1"),
    feature = "sh0"
))]
pub const SH_DEGREE: usize = 0;

#[cfg(all(
    not(feature = "sh4"),
    not(feature = "sh3"),
    not(feature = "sh2"),
    not(feature = "sh1"),
    not(feature = "sh0")
))]
pub const SH_DEGREE: usize = 0;

pub const SH_CHANNELS: usize = 3;
pub const SH_COEFF_COUNT_PER_CHANNEL: usize = num_sh_coefficients(SH_DEGREE);
pub const SH_COEFF_COUNT: usize = pad_4(SH_COEFF_COUNT_PER_CHANNEL * SH_CHANNELS);

pub const HALF_SH_COEFF_COUNT: usize = SH_COEFF_COUNT / 2;
pub const PADDED_HALF_SH_COEFF_COUNT: usize = pad_4(HALF_SH_COEFF_COUNT);

// #[cfg(feature = "f16")]
// pub const SH_VEC4_PLANES: usize = PADDED_HALF_SH_COEFF_COUNT / 4;
pub const SH_VEC4_PLANES: usize = SH_COEFF_COUNT / 4;

// #[cfg(feature = "f16")]
// #[derive(
//     Clone,
//     Copy,
//     Debug,
//     PartialEq,
//     Reflect,
//     ShaderType,
//     Pod,
//     Zeroable,
//     Serialize,
//     Deserialize,
// )]
// #[repr(C)]
// pub struct SphericalHarmonicCoefficients {
//     #[reflect(ignore)]
//     #[serde(serialize_with = "coefficients_serializer", deserialize_with = "coefficients_deserializer")]
//     pub coefficients: [u32; HALF_SH_COEFF_COUNT],
// }

#[allow(dead_code)]
#[derive(
    Clone, Copy, Debug, PartialEq, Reflect, ShaderType, Pod, Zeroable, Serialize, Deserialize,
)]
#[repr(C)]
pub struct SphericalHarmonicCoefficients {
    #[serde(
        serialize_with = "coefficients_serializer",
        deserialize_with = "coefficients_deserializer"
    )]
    pub coefficients: [f32; SH_COEFF_COUNT],
}

// #[cfg(feature = "f16")]
// impl Default for SphericalHarmonicCoefficients {
//     fn default() -> Self {
//         Self {
//             coefficients: [0; HALF_SH_COEFF_COUNT],
//         }
//     }
// }

impl Default for SphericalHarmonicCoefficients {
    fn default() -> Self {
        Self {
            coefficients: [0.0; SH_COEFF_COUNT],
        }
    }
}

impl SphericalHarmonicCoefficients {
    // #[cfg(feature = "f16")]
    // pub fn set(&mut self, index: usize, value: f32) {
    //     let quantized = f16::from_f32(value).to_bits();
    //     self.coefficients[index / 2] = match index % 2 {
    //         0 => (self.coefficients[index / 2] & 0xffff0000) | (quantized as u32),
    //         1 => (self.coefficients[index / 2] & 0x0000ffff) | ((quantized as u32) << 16),
    //         _ => unreachable!(),
    //     };
    // }

    pub fn set(&mut self, index: usize, value: f32) {
        self.coefficients[index] = value;
    }
}

// #[cfg(feature = "f16")]
// fn coefficients_serializer<S>(n: &[u32; HALF_SH_COEFF_COUNT], s: S) -> Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     let mut tup = s.serialize_tuple(HALF_SH_COEFF_COUNT)?;
//     for &x in n.iter() {
//         tup.serialize_element(&x)?;
//     }

//     tup.end()
// }

// #[cfg(feature = "f16")]
// fn coefficients_deserializer<'de, D>(d: D) -> Result<[u32; HALF_SH_COEFF_COUNT], D::Error>
// where
//     D: serde::Deserializer<'de>,
// {
//     struct CoefficientsVisitor;

//     impl<'de> serde::de::Visitor<'de> for CoefficientsVisitor {
//         type Value = [u32; HALF_SH_COEFF_COUNT];

//         fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//             formatter.write_str("an array of floats")
//         }

//         fn visit_seq<A>(self, mut seq: A) -> Result<[u32; HALF_SH_COEFF_COUNT], A::Error>
//         where
//             A: serde::de::SeqAccess<'de>,
//         {
//             let mut coefficients = [0; HALF_SH_COEFF_COUNT];

//             for (i, coefficient) in coefficients.iter_mut().enumerate().take(SH_COEFF_COUNT) {
//                 *coefficient = seq
//                     .next_element()?
//                     .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
//             }
//             Ok(coefficients)
//         }
//     }

//     d.deserialize_tuple(HALF_SH_COEFF_COUNT, CoefficientsVisitor)
// }

fn coefficients_serializer<S>(n: &[f32; SH_COEFF_COUNT], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut tup = s.serialize_tuple(SH_COEFF_COUNT)?;
    for &x in n.iter() {
        tup.serialize_element(&x)?;
    }

    tup.end()
}

fn coefficients_deserializer<'de, D>(d: D) -> Result<[f32; SH_COEFF_COUNT], D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct CoefficientsVisitor;

    impl<'de> serde::de::Visitor<'de> for CoefficientsVisitor {
        type Value = [f32; SH_COEFF_COUNT];

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an array of floats")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<[f32; SH_COEFF_COUNT], A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut coefficients = [0.0; SH_COEFF_COUNT];
            let mut index = 0usize;

            while let Some(value) = seq.next_element()? {
                if index < SH_COEFF_COUNT {
                    coefficients[index] = value;
                }
                index += 1;
            }
            Ok(coefficients)
        }
    }

    d.deserialize_seq(CoefficientsVisitor)
}
