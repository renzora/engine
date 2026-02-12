//! Post-processing component data types
//!
//! Individual, toggleable post-processing components that were previously
//! part of the monolithic WorldEnvironmentData.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::environment::{PanoramaSkyData, ProceduralSkyData, SkyMode};

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

/// Skybox / sky background settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SkyboxData {
    pub sky_mode: SkyMode,
    pub clear_color: (f32, f32, f32),
    pub procedural_sky: ProceduralSkyData,
    pub panorama_sky: PanoramaSkyData,
}

impl Default for SkyboxData {
    fn default() -> Self {
        Self {
            sky_mode: SkyMode::default(),
            clear_color: (0.4, 0.6, 0.9),
            procedural_sky: ProceduralSkyData::default(),
            panorama_sky: PanoramaSkyData {
                panorama_path: String::new(),
                rotation: 0.0,
                energy: 1.0,
            },
        }
    }
}

/// Fog settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct FogData {
    pub enabled: bool,
    pub color: (f32, f32, f32),
    pub start: f32,
    pub end: f32,
}

impl Default for FogData {
    fn default() -> Self {
        Self {
            enabled: false,
            color: (0.5, 0.5, 0.5),
            start: 10.0,
            end: 100.0,
        }
    }
}

/// Anti-aliasing settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AntiAliasingData {
    pub msaa_samples: u8,
    pub fxaa_enabled: bool,
}

impl Default for AntiAliasingData {
    fn default() -> Self {
        Self {
            msaa_samples: 4,
            fxaa_enabled: false,
        }
    }
}

/// Screen-space ambient occlusion settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AmbientOcclusionData {
    pub enabled: bool,
    pub intensity: f32,
    pub radius: f32,
}

impl Default for AmbientOcclusionData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 1.0,
            radius: 0.5,
        }
    }
}

/// Screen-space reflections settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct ReflectionsData {
    pub enabled: bool,
    pub intensity: f32,
    pub max_steps: u32,
}

impl Default for ReflectionsData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.5,
            max_steps: 64,
        }
    }
}

/// Bloom settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct BloomData {
    pub enabled: bool,
    pub intensity: f32,
    pub threshold: f32,
}

impl Default for BloomData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.15,
            threshold: 1.0,
        }
    }
}

/// Tonemapping settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TonemappingData {
    pub mode: TonemappingMode,
    pub ev100: f32,
}

impl Default for TonemappingData {
    fn default() -> Self {
        Self {
            mode: TonemappingMode::Reinhard,
            ev100: 9.7,
        }
    }
}

/// Depth of field settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct DepthOfFieldData {
    pub enabled: bool,
    pub focal_distance: f32,
    pub aperture: f32,
}

impl Default for DepthOfFieldData {
    fn default() -> Self {
        Self {
            enabled: false,
            focal_distance: 10.0,
            aperture: 0.05,
        }
    }
}

/// Motion blur settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MotionBlurData {
    pub enabled: bool,
    pub intensity: f32,
}

impl Default for MotionBlurData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.5,
        }
    }
}

/// Ambient light settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AmbientLightData {
    pub color: (f32, f32, f32),
    pub brightness: f32,
}

impl Default for AmbientLightData {
    fn default() -> Self {
        Self {
            color: (1.0, 1.0, 1.0),
            brightness: 300.0,
        }
    }
}

// ── New post-processing data types ──────────────────────────────────────

/// Temporal Anti-Aliasing settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TaaData {
    pub enabled: bool,
    pub reset: bool,
}

impl Default for TaaData {
    fn default() -> Self {
        Self {
            enabled: false,
            reset: false,
        }
    }
}

/// SMAA preset quality levels
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SmaaPresetMode {
    Low,
    Medium,
    #[default]
    High,
    Ultra,
}

/// SMAA (Subpixel Morphological Anti-Aliasing) settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SmaaData {
    pub enabled: bool,
    pub preset: SmaaPresetMode,
}

impl Default for SmaaData {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: SmaaPresetMode::High,
        }
    }
}

/// Contrast Adaptive Sharpening settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct CasData {
    pub enabled: bool,
    pub sharpening_strength: f32,
    pub denoise: bool,
}

impl Default for CasData {
    fn default() -> Self {
        Self {
            enabled: false,
            sharpening_strength: 0.6,
            denoise: false,
        }
    }
}

/// Chromatic Aberration settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct ChromaticAberrationData {
    pub enabled: bool,
    pub intensity: f32,
    pub max_samples: u32,
}

impl Default for ChromaticAberrationData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.02,
            max_samples: 8,
        }
    }
}

/// Auto Exposure settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AutoExposureData {
    pub enabled: bool,
    pub speed_brighten: f32,
    pub speed_darken: f32,
    pub range_min: f32,
    pub range_max: f32,
}

impl Default for AutoExposureData {
    fn default() -> Self {
        Self {
            enabled: false,
            speed_brighten: 3.0,
            speed_darken: 1.0,
            range_min: -8.0,
            range_max: 8.0,
        }
    }
}

/// Volumetric Fog settings (camera + light)
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VolumetricFogData {
    pub enabled: bool,
    pub ambient_color: (f32, f32, f32),
    pub ambient_intensity: f32,
    pub step_count: u32,
    pub volumetric_light: bool,
}

impl Default for VolumetricFogData {
    fn default() -> Self {
        Self {
            enabled: false,
            ambient_color: (1.0, 1.0, 1.0),
            ambient_intensity: 0.1,
            step_count: 64,
            volumetric_light: true,
        }
    }
}

/// Vignette effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VignetteData {
    pub enabled: bool,
    pub intensity: f32,
    pub radius: f32,
    pub smoothness: f32,
    pub color: (f32, f32, f32),
}

impl Default for VignetteData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.5,
            radius: 0.8,
            smoothness: 0.3,
            color: (0.0, 0.0, 0.0),
        }
    }
}

/// Film Grain effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct FilmGrainData {
    pub enabled: bool,
    pub intensity: f32,
    pub grain_size: f32,
}

impl Default for FilmGrainData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.1,
            grain_size: 1.0,
        }
    }
}

/// Pixelation effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PixelationData {
    pub enabled: bool,
    pub pixel_size: f32,
}

impl Default for PixelationData {
    fn default() -> Self {
        Self {
            enabled: false,
            pixel_size: 4.0,
        }
    }
}

/// CRT display effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct CrtData {
    pub enabled: bool,
    pub scanline_intensity: f32,
    pub curvature: f32,
    pub chromatic_amount: f32,
    pub vignette_amount: f32,
}

impl Default for CrtData {
    fn default() -> Self {
        Self {
            enabled: false,
            scanline_intensity: 0.3,
            curvature: 0.02,
            chromatic_amount: 0.005,
            vignette_amount: 0.3,
        }
    }
}

/// God Rays (light shaft) effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct GodRaysData {
    pub enabled: bool,
    pub intensity: f32,
    pub decay: f32,
    pub density: f32,
    pub num_samples: u32,
    pub light_screen_pos: (f32, f32),
}

impl Default for GodRaysData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.5,
            decay: 0.97,
            density: 1.0,
            num_samples: 64,
            light_screen_pos: (0.5, 0.3),
        }
    }
}

/// Gaussian Blur effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct GaussianBlurData {
    pub enabled: bool,
    pub sigma: f32,
    pub kernel_size: u32,
}

impl Default for GaussianBlurData {
    fn default() -> Self {
        Self {
            enabled: false,
            sigma: 2.0,
            kernel_size: 9,
        }
    }
}

/// Palette Quantization (color reduction) settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PaletteQuantizationData {
    pub enabled: bool,
    pub num_colors: u32,
    pub dithering: f32,
}

impl Default for PaletteQuantizationData {
    fn default() -> Self {
        Self {
            enabled: false,
            num_colors: 16,
            dithering: 0.5,
        }
    }
}

/// Distortion / Heat Haze effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct DistortionData {
    pub enabled: bool,
    pub intensity: f32,
    pub speed: f32,
    pub scale: f32,
}

impl Default for DistortionData {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.01,
            speed: 1.0,
            scale: 10.0,
        }
    }
}

/// Underwater / Rain on Lens effect settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UnderwaterData {
    pub enabled: bool,
    pub distortion: f32,
    pub tint_color: (f32, f32, f32),
    pub tint_strength: f32,
    pub wave_speed: f32,
    pub wave_scale: f32,
}

impl Default for UnderwaterData {
    fn default() -> Self {
        Self {
            enabled: false,
            distortion: 0.03,
            tint_color: (0.1, 0.4, 0.6),
            tint_strength: 0.4,
            wave_speed: 1.0,
            wave_scale: 5.0,
        }
    }
}
