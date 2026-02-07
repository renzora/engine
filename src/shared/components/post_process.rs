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
