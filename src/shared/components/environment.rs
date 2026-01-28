//! World environment component data types
//!
//! Defines sky, fog, ambient lighting, and post-processing settings.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Sky rendering mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SkyMode {
    /// Solid color background (uses clear_color)
    #[default]
    Color,
    /// Procedural gradient sky with sun
    Procedural,
    /// HDR panorama skybox
    Panorama,
}

/// Procedural sky parameters
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct ProceduralSkyData {
    /// Top color of the sky gradient (RGB tuple for compatibility)
    pub sky_top_color: (f32, f32, f32),
    /// Horizon color of the sky
    pub sky_horizon_color: (f32, f32, f32),
    /// Bottom/ground color
    pub ground_bottom_color: (f32, f32, f32),
    /// Horizon color for ground
    pub ground_horizon_color: (f32, f32, f32),
    /// Sun position as angles (azimuth in degrees)
    pub sun_angle_azimuth: f32,
    /// Sun elevation in degrees
    pub sun_angle_elevation: f32,
    /// Sun disk size (0 = no sun disk visible)
    pub sun_disk_scale: f32,
    /// Sun color
    pub sun_color: (f32, f32, f32),
    /// Sun intensity/energy
    pub sun_energy: f32,
    /// Curve for sky gradient blending (0.0-1.0, lower = sharper horizon)
    pub sky_curve: f32,
    /// Curve for ground gradient blending
    pub ground_curve: f32,
}

impl Default for ProceduralSkyData {
    fn default() -> Self {
        Self {
            sky_top_color: (0.15, 0.35, 0.65),       // Deep blue
            sky_horizon_color: (0.55, 0.70, 0.85),   // Light blue
            ground_bottom_color: (0.2, 0.17, 0.13),  // Brown/dirt
            ground_horizon_color: (0.55, 0.55, 0.52), // Light gray
            sun_angle_azimuth: 0.0,
            sun_angle_elevation: 45.0,
            sun_disk_scale: 1.0,
            sun_color: (1.0, 0.95, 0.85),
            sun_energy: 1.0,
            sky_curve: 0.15,
            ground_curve: 0.02,
        }
    }
}

/// HDR panorama sky parameters
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct PanoramaSkyData {
    /// Path to HDR/EXR file (relative to project)
    pub panorama_path: String,
    /// Rotation of the panorama in degrees
    pub rotation: f32,
    /// Energy/brightness multiplier
    pub energy: f32,
}

/// Tonemapping modes available
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum TonemappingMode {
    None,
    #[default]
    Reinhard,
    ReinhardLuminance,
    AcesFitted,
    AgX,
    SomewhatBoringDisplayTransform,
    TonyMcMapface,
    BlenderFilmic,
}

/// World environment configuration (ambient light, fog, sky, post-processing)
/// This is a data struct stored inside WorldEnvironmentMarker component.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WorldEnvironmentData {
    // Ambient Light (RGB tuples for compatibility with existing inspector code)
    pub ambient_color: (f32, f32, f32),
    pub ambient_brightness: f32,
    // Sky / Background
    pub sky_mode: SkyMode,
    pub clear_color: (f32, f32, f32),
    pub procedural_sky: ProceduralSkyData,
    pub panorama_sky: PanoramaSkyData,
    // Fog
    pub fog_enabled: bool,
    pub fog_color: (f32, f32, f32),
    pub fog_start: f32,
    pub fog_end: f32,
    // Anti-aliasing
    pub msaa_samples: u8, // 1, 2, 4, 8
    pub fxaa_enabled: bool,
    // Screen Space Ambient Occlusion
    pub ssao_enabled: bool,
    pub ssao_intensity: f32,
    pub ssao_radius: f32,
    // Screen Space Reflections
    pub ssr_enabled: bool,
    pub ssr_intensity: f32,
    pub ssr_max_steps: u32,
    // Bloom
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    // Tonemapping
    pub tonemapping: TonemappingMode,
    /// Exposure value in EV100 (higher = darker, typical outdoor ~9.7)
    pub ev100: f32,
    // Depth of Field
    pub dof_enabled: bool,
    pub dof_focal_distance: f32,
    pub dof_aperture: f32,
    // Motion Blur
    pub motion_blur_enabled: bool,
    pub motion_blur_intensity: f32,
}

impl Default for WorldEnvironmentData {
    fn default() -> Self {
        Self {
            // Ambient Light
            ambient_color: (1.0, 1.0, 1.0),
            ambient_brightness: 300.0,
            // Sky / Background
            sky_mode: SkyMode::default(),
            clear_color: (0.4, 0.6, 0.9),
            procedural_sky: ProceduralSkyData::default(),
            panorama_sky: PanoramaSkyData {
                panorama_path: String::new(),
                rotation: 0.0,
                energy: 1.0,
            },
            // Fog
            fog_enabled: false,
            fog_color: (0.5, 0.5, 0.5),
            fog_start: 10.0,
            fog_end: 100.0,
            // Anti-aliasing
            msaa_samples: 4,
            fxaa_enabled: false,
            // SSAO
            ssao_enabled: false,
            ssao_intensity: 1.0,
            ssao_radius: 0.5,
            // SSR
            ssr_enabled: false,
            ssr_intensity: 0.5,
            ssr_max_steps: 64,
            // Bloom
            bloom_enabled: false,
            bloom_intensity: 0.15,
            bloom_threshold: 1.0,
            // Tonemapping
            tonemapping: TonemappingMode::Reinhard,
            ev100: 9.7,
            // Depth of Field
            dof_enabled: false,
            dof_focal_distance: 10.0,
            dof_aperture: 0.05,
            // Motion Blur
            motion_blur_enabled: false,
            motion_blur_intensity: 0.5,
        }
    }
}
