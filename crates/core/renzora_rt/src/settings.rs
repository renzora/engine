use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Quality preset for the RT lighting pipeline.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RtQuality {
    /// Half-res GI, 32 ray steps, 2 spatial denoise iterations.
    Low,
    /// Half-res GI, 64 ray steps, 3 spatial denoise iterations.
    #[default]
    Medium,
    /// Full-res GI, 64 ray steps, 4 spatial denoise iterations.
    High,
    /// Full-res GI, 128 ray steps, 5 spatial denoise iterations.
    Ultra,
}

impl RtQuality {
    pub const ALL: [RtQuality; 4] = [
        RtQuality::Low,
        RtQuality::Medium,
        RtQuality::High,
        RtQuality::Ultra,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RtQuality::Low => "Low",
            RtQuality::Medium => "Medium",
            RtQuality::High => "High",
            RtQuality::Ultra => "Ultra",
        }
    }
}

/// Screen-space ray-traced lighting component.
///
/// Add this to a camera entity to enable the RT lighting pipeline.
/// Requires depth prepass and motion vector prepass (auto-inserted via `#[require]`).
#[derive(Component, Reflect, Clone, Debug, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct RtLighting {
    /// Master enable for the entire RT lighting system.
    pub enabled: bool,

    // -- Global Illumination --
    pub gi_enabled: bool,
    /// Intensity multiplier for indirect diffuse lighting (0.0 - 2.0).
    pub gi_intensity: f32,
    /// Maximum number of Hi-Z ray march steps per ray.
    pub gi_max_ray_steps: u32,
    /// Maximum world-space distance for GI rays.
    pub gi_max_distance: f32,
    /// Screen-space thickness used for hit detection during ray marching.
    pub gi_thickness: f32,

    // -- Reflections --
    pub reflections_enabled: bool,
    /// Intensity multiplier for specular reflections.
    pub reflections_intensity: f32,

    // -- Contact Shadows --
    pub shadows_enabled: bool,
    /// Maximum steps for screen-space contact shadow rays.
    pub shadow_max_steps: u32,

    // -- Denoise --
    pub denoise_temporal: bool,
    /// Number of spatial (A-Trous wavelet) denoise iterations (0-5).
    pub denoise_spatial_iterations: u32,

    // -- Quality --
    pub quality: RtQuality,

    /// Set to `true` to clear temporal history (e.g. after a camera cut).
    /// Automatically reset to `false` after one frame.
    pub reset: bool,
}

impl Default for RtLighting {
    fn default() -> Self {
        Self {
            enabled: true,
            gi_enabled: true,
            gi_intensity: 0.5,
            gi_max_ray_steps: 64,
            gi_max_distance: 50.0,
            gi_thickness: 0.5,
            reflections_enabled: true,
            reflections_intensity: 1.0,
            shadows_enabled: true,
            shadow_max_steps: 16,
            denoise_temporal: true,
            denoise_spatial_iterations: 3,
            quality: RtQuality::default(),
            reset: true, // No history on first frame
        }
    }
}

impl RtLighting {
    /// Apply a quality preset, overwriting the relevant per-feature settings.
    pub fn apply_quality(&mut self, quality: RtQuality) {
        self.quality = quality;
        match quality {
            RtQuality::Low => {
                self.gi_max_ray_steps = 32;
                self.denoise_spatial_iterations = 2;
            }
            RtQuality::Medium => {
                self.gi_max_ray_steps = 64;
                self.denoise_spatial_iterations = 3;
            }
            RtQuality::High => {
                self.gi_max_ray_steps = 64;
                self.denoise_spatial_iterations = 4;
            }
            RtQuality::Ultra => {
                self.gi_max_ray_steps = 128;
                self.denoise_spatial_iterations = 5;
            }
        }
    }

    /// Whether the trace resolution should be half the viewport.
    pub fn half_res(&self) -> bool {
        matches!(self.quality, RtQuality::Low | RtQuality::Medium)
    }
}

/// GPU-friendly push constants sent to every RT compute dispatch.
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct RtPushConstants {
    pub frame_index: u32,
    pub reset: u32,
    pub gi_max_ray_steps: u32,
    pub gi_max_distance_bits: u32, // f32 bits
    pub gi_thickness_bits: u32,    // f32 bits
    pub gi_intensity_bits: u32,    // f32 bits
    pub refl_intensity_bits: u32,  // f32 bits
    pub flags: u32, // bit 0 = gi, bit 1 = refl, bit 2 = shadows, bit 3 = half_res
    // Primary directional light direction (toward light, world space)
    pub light_dir_x: f32,
    pub light_dir_y: f32,
    pub light_dir_z: f32,
    pub shadow_max_steps: u32,
}

impl RtPushConstants {
    pub fn from_settings(
        settings: &RtLighting,
        frame_index: u32,
        light_dir: [f32; 3],
    ) -> Self {
        let flags = (settings.gi_enabled as u32)
            | ((settings.reflections_enabled as u32) << 1)
            | ((settings.shadows_enabled as u32) << 2)
            | ((settings.half_res() as u32) << 3);
        Self {
            frame_index,
            reset: settings.reset as u32,
            gi_max_ray_steps: settings.gi_max_ray_steps,
            gi_max_distance_bits: settings.gi_max_distance.to_bits(),
            gi_thickness_bits: settings.gi_thickness.to_bits(),
            gi_intensity_bits: settings.gi_intensity.to_bits(),
            refl_intensity_bits: settings.reflections_intensity.to_bits(),
            flags,
            light_dir_x: light_dir[0],
            light_dir_y: light_dir[1],
            light_dir_z: light_dir[2],
            shadow_max_steps: settings.shadow_max_steps,
        }
    }
}
