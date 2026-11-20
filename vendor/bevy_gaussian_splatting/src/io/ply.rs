use core::panic;
use std::io::BufRead;

use bevy_interleave::prelude::Planar;
use ply_rs::{
    parser::Parser,
    ply::{Property, PropertyAccess},
};

use crate::{
    gaussian::formats::{
        planar_3d::{Gaussian3d, PlanarGaussian3d},
        planar_4d::{Gaussian4d, PlanarGaussian4d},
    },
    material::{
        spherical_harmonics::{SH_CHANNELS, SH_COEFF_COUNT, SH_COEFF_COUNT_PER_CHANNEL},
        spherindrical_harmonics::SH_4D_COEFF_COUNT,
    },
};

pub const MAX_SIZE_VARIANCE: f32 = 4.0;

/// renzora patch: a plain colored point cloud parsed from a non-3DGS `.ply`
/// (CloudCompare exports, Sketchfab's converted downloads, LiDAR tools —
/// `x/y/z` plus optional `red/green/blue`, no gaussian parameters).
/// [`parse_ply_3d`] falls back to synthesizing an isotropic gaussian per point
/// instead of failing with "missing required properties", since these are the
/// most common "splat-looking" files in the wild.
#[derive(Clone, Copy)]
struct PlyPoint {
    position: [f32; 3],
    color: [f32; 3],
}

impl PropertyAccess for PlyPoint {
    fn new() -> Self {
        Self {
            position: [0.0; 3],
            // Mid-gray for colorless clouds so they render visibly instead of
            // as black (SH DC of 0.5 is the neutral color).
            color: [0.5; 3],
        }
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_ref(), property) {
            ("x", Property::Float(v)) => self.position[0] = v,
            ("y", Property::Float(v)) => self.position[1] = v,
            ("z", Property::Float(v)) => self.position[2] = v,
            ("red", Property::UChar(v)) => self.color[0] = v as f32 / 255.0,
            ("green", Property::UChar(v)) => self.color[1] = v as f32 / 255.0,
            ("blue", Property::UChar(v)) => self.color[2] = v as f32 / 255.0,
            ("red", Property::Float(v)) => self.color[0] = v,
            ("green", Property::Float(v)) => self.color[1] = v,
            ("blue", Property::Float(v)) => self.color[2] = v,
            (_, _) => {}
        }
    }
}

/// Turn plain points into renderable isotropic gaussians: DC spherical
/// harmonics from the vertex color, full opacity, identity rotation, and a
/// radius estimated from point density (bounding-box volume per point) so the
/// splats roughly close the gaps between neighbors regardless of the scan's
/// units. Per-entity `CloudSettings::global_scale` (the inspector's Splat
/// Scale) remains the tuning knob when the estimate reads too fat or too thin.
fn synthesize_isotropic_gaussians(points: Vec<PlyPoint>) -> Vec<Gaussian3d> {
    // 1 / (2 * sqrt(pi)) — the constant DC basis factor of spherical
    // harmonics; 3DGS stores color as `0.5 + SH_C0 * f_dc`.
    const SH_C0: f32 = 0.282_094_79;

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &points {
        for i in 0..3 {
            min[i] = min[i].min(p.position[i]);
            max[i] = max[i].max(p.position[i]);
        }
    }
    let volume: f32 = (0..3).map(|i| (max[i] - min[i]).max(1e-4)).product();
    let spacing = (volume / points.len().max(1) as f32).cbrt();
    // Radius slightly above the mean spacing so neighboring splats overlap
    // into a closed surface instead of a dot field.
    let radius = (spacing * 1.5).max(1e-4);

    points
        .into_iter()
        .map(|p| {
            let mut gaussian = Gaussian3d::default();
            gaussian.position_visibility.position = p.position;
            for (i, channel) in p.color.iter().enumerate() {
                gaussian.spherical_harmonic.set(i, (channel - 0.5) / SH_C0);
            }
            gaussian.scale_opacity.scale = [radius; 3];
            gaussian.scale_opacity.opacity = 1.0;
            gaussian.rotation.rotation = [1.0, 0.0, 0.0, 0.0];
            gaussian
        })
        .collect()
}

impl PropertyAccess for Gaussian3d {
    fn new() -> Self {
        Gaussian3d::default()
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_ref(), property) {
            ("x", Property::Float(v)) => self.position_visibility.position[0] = v,
            ("y", Property::Float(v)) => self.position_visibility.position[1] = v,
            ("z", Property::Float(v)) => self.position_visibility.position[2] = v,
            ("visibility", Property::Float(v)) => self.position_visibility.visibility = v,
            ("f_dc_0", Property::Float(v)) => self.spherical_harmonic.set(0, v),
            ("f_dc_1", Property::Float(v)) => self.spherical_harmonic.set(1, v),
            ("f_dc_2", Property::Float(v)) => self.spherical_harmonic.set(2, v),
            ("scale_0", Property::Float(v)) => self.scale_opacity.scale[0] = v,
            ("scale_1", Property::Float(v)) => self.scale_opacity.scale[1] = v,
            ("scale_2", Property::Float(v)) => self.scale_opacity.scale[2] = v,
            ("opacity", Property::Float(v)) => {
                self.scale_opacity.opacity = 1.0 / (1.0 + (-v).exp())
            }
            ("rot_0", Property::Float(v)) => self.rotation.rotation[0] = v,
            ("rot_1", Property::Float(v)) => self.rotation.rotation[1] = v,
            ("rot_2", Property::Float(v)) => self.rotation.rotation[2] = v,
            ("rot_3", Property::Float(v)) => self.rotation.rotation[3] = v,
            (_, Property::Float(v)) if key.starts_with("f_rest_") => {
                let i = key[7..].parse::<usize>().unwrap();

                // renzora patch: `f_rest_*` is laid out planar with
                // REST_PER_CHANNEL (15 at sh3) coefficients per channel — the
                // DC term lives in `f_dc_*`, not here. Upstream divided by
                // SH_COEFF_COUNT_PER_CHANNEL (16, DC included), which
                // misassigned every coefficient from f_rest_15 on (G's first
                // coefficient landed in R's slot 1, colliding with f_rest_0).
                const REST_PER_CHANNEL: usize = SH_COEFF_COUNT_PER_CHANNEL.saturating_sub(1);
                if REST_PER_CHANNEL > 0 {
                    let channel = i / REST_PER_CHANNEL;
                    let coefficient = (i % REST_PER_CHANNEL) + 1;

                    let interleaved_idx = coefficient * SH_CHANNELS + channel;

                    if channel < SH_CHANNELS && interleaved_idx < SH_COEFF_COUNT {
                        self.spherical_harmonic.set(interleaved_idx, v);
                    } else {
                        // TODO: convert higher degree SH to lower degree SH
                    }
                }
            }
            (_, _) => {}
        }
    }
}

pub fn parse_ply_3d(mut reader: &mut dyn BufRead) -> Result<PlanarGaussian3d, std::io::Error> {
    let gaussian_parser = Parser::<Gaussian3d>::new();
    let header = gaussian_parser.read_header(&mut reader)?;

    let mut cloud = Vec::new();

    let required_properties = vec![
        "x", "y", "z", "f_dc_0", "f_dc_1", "f_dc_2", "scale_0", "scale_1", "opacity", "rot_0",
        "rot_1", "rot_2", "rot_3",
    ];
    let mut required_property_count = required_properties.len();

    for (_key, element) in &header.elements {
        if element.name == "vertex" {
            for (key, _prop) in &element.properties {
                required_property_count -= required_properties.contains(&key.as_str()) as usize;
            }

            if required_property_count > 0 {
                // renzora patch: no gaussian parameters, but positions exist →
                // treat the file as a plain (optionally colored) point cloud
                // and synthesize an isotropic splat per point rather than
                // rejecting it. Synthesized values are already in final
                // (linear-scale, normalized-rotation) form, so skip the
                // log-scale/quat post-processing below.
                let has_positions = ["x", "y", "z"]
                    .iter()
                    .all(|required| element.properties.iter().any(|(key, _)| key == required));
                if !has_positions {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "missing required properties",
                    ));
                }

                let point_parser = Parser::<PlyPoint>::new();
                let points =
                    point_parser.read_payload_for_element(&mut reader, element, &header)?;
                let mut cloud = synthesize_isotropic_gaussians(points);

                let pad = 32 - (cloud.len() % 32);
                cloud.extend(std::iter::repeat_n(Gaussian3d::default(), pad));

                return Ok(PlanarGaussian3d::from_interleaved(cloud));
            }

            cloud = gaussian_parser.read_payload_for_element(&mut reader, element, &header)?;
        }
    }

    for gaussian in &mut cloud {
        // TODO: add automatic scaling normalization detection (e.g. don't normalize twice)
        let mean_scale = (gaussian.scale_opacity.scale[0]
            + gaussian.scale_opacity.scale[1]
            + gaussian.scale_opacity.scale[2])
            / 3.0;
        for i in 0..3 {
            gaussian.scale_opacity.scale[i] = gaussian.scale_opacity.scale[i]
                .max(mean_scale - MAX_SIZE_VARIANCE)
                .min(mean_scale + MAX_SIZE_VARIANCE)
                .exp();
        }

        let norm = (0..4)
            .map(|i| gaussian.rotation.rotation[i].powf(2.0))
            .sum::<f32>()
            .sqrt();
        for i in 0..4 {
            gaussian.rotation.rotation[i] /= norm;
        }
    }

    // pad with empty gaussians to multiple of 32
    let pad = 32 - (cloud.len() % 32);
    cloud.extend(std::iter::repeat_n(Gaussian3d::default(), pad));

    Ok(PlanarGaussian3d::from_interleaved(cloud))
}

impl PropertyAccess for Gaussian4d {
    fn new() -> Self {
        Gaussian4d::default()
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_ref(), property) {
            ("x", Property::Float(v)) => self.position_visibility.position[0] = v,
            ("y", Property::Float(v)) => self.position_visibility.position[1] = v,
            ("z", Property::Float(v)) => self.position_visibility.position[2] = v,
            ("visibility", Property::Float(v)) => self.position_visibility.visibility = v,

            ("t", Property::Float(v)) => self.timestamp_timescale.timestamp = v,
            ("st", Property::Float(v)) => self.timestamp_timescale.timescale = v,

            (_, Property::Float(v)) if key.starts_with("feat_") => {
                let channel = match key.chars().nth(5).unwrap() {
                    'r' => 0,
                    'g' => 1,
                    'b' => 2,
                    _ => panic!("invalid feature channel, expected r, g, or b"),
                };
                let i = key[7..].parse::<usize>().unwrap();
                let interleaved_idx = i * SH_CHANNELS + channel;

                if interleaved_idx < SH_4D_COEFF_COUNT {
                    self.spherindrical_harmonic.set(interleaved_idx, v);
                } else {
                    // TODO: handle higher-degree if needed
                }
            }

            ("sx", Property::Float(v)) => self.scale_opacity.scale[0] = v,
            ("sy", Property::Float(v)) => self.scale_opacity.scale[1] = v,
            ("sz", Property::Float(v)) => self.scale_opacity.scale[2] = v,
            ("opacity", Property::Float(v)) => self.scale_opacity.opacity = v,

            ("rot_x", Property::Float(v)) => self.isotropic_rotations.rotation[0] = v,
            ("rot_y", Property::Float(v)) => self.isotropic_rotations.rotation[1] = v,
            ("rot_z", Property::Float(v)) => self.isotropic_rotations.rotation[2] = v,
            ("rot_w", Property::Float(v)) => self.isotropic_rotations.rotation[3] = v,

            ("rot_r_x", Property::Float(v)) => self.isotropic_rotations.rotation_r[0] = v,
            ("rot_r_y", Property::Float(v)) => self.isotropic_rotations.rotation_r[1] = v,
            ("rot_r_z", Property::Float(v)) => self.isotropic_rotations.rotation_r[2] = v,
            ("rot_r_w", Property::Float(v)) => self.isotropic_rotations.rotation_r[3] = v,
            _ => {}
        }
    }
}

pub fn parse_ply_4d(mut reader: &mut dyn BufRead) -> Result<PlanarGaussian4d, std::io::Error> {
    let parser = Parser::<Gaussian4d>::new();
    let header = parser.read_header(&mut reader)?;

    let mut cloud = Vec::new();

    let required_properties = vec![
        "x", "y", "z", "t", "st", "sx", "sy", "sz", "opacity", "rot_x", "rot_y", "rot_z", "rot_w",
        "rot_r_x", "rot_r_y", "rot_r_z", "rot_r_w",
    ];
    let mut required_property_count = required_properties.len();

    for (_key, element) in &header.elements {
        if element.name == "vertex" {
            for (key, _prop) in &element.properties {
                required_property_count -= required_properties.contains(&key.as_str()) as usize;
            }

            if required_property_count > 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "missing required properties",
                ));
            }

            cloud = parser.read_payload_for_element(&mut reader, element, &header)?;
        }
    }

    for g in &mut cloud {
        let norm = g
            .isotropic_rotations
            .rotation
            .iter()
            .map(|v| v.powi(2))
            .sum::<f32>()
            .sqrt();

        for v in &mut g.isotropic_rotations.rotation {
            *v /= norm;
        }

        let norm = g
            .isotropic_rotations
            .rotation_r
            .iter()
            .map(|v| v.powi(2))
            .sum::<f32>()
            .sqrt();

        for v in &mut g.isotropic_rotations.rotation_r {
            *v /= norm;
        }

        // TODO: normalize timescale between 0 and 1
    }

    // pad to multiple of 32
    let pad = 32 - (cloud.len() % 32);
    cloud.extend(std::iter::repeat_n(Gaussian4d::default(), pad));

    Ok(PlanarGaussian4d::from_interleaved(cloud))
}
