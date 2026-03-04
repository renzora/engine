//! World environment component data types
//!
//! Defines sky, fog, ambient lighting, and post-processing settings.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Sky rendering mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SkyMode {
    /// Solid color background (uses clear_color)
    #[default]
    Color,
    /// Procedural gradient sky
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
    /// Curve for sky gradient blending (0.0-1.0, lower = sharper horizon)
    pub sky_curve: f32,
    /// Curve for ground gradient blending
    pub ground_curve: f32,
}

impl Hash for ProceduralSkyData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sky_top_color.0.to_bits().hash(state);
        self.sky_top_color.1.to_bits().hash(state);
        self.sky_top_color.2.to_bits().hash(state);
        self.sky_horizon_color.0.to_bits().hash(state);
        self.sky_horizon_color.1.to_bits().hash(state);
        self.sky_horizon_color.2.to_bits().hash(state);
        self.ground_bottom_color.0.to_bits().hash(state);
        self.ground_bottom_color.1.to_bits().hash(state);
        self.ground_bottom_color.2.to_bits().hash(state);
        self.ground_horizon_color.0.to_bits().hash(state);
        self.ground_horizon_color.1.to_bits().hash(state);
        self.ground_horizon_color.2.to_bits().hash(state);
        self.sky_curve.to_bits().hash(state);
        self.ground_curve.to_bits().hash(state);
    }
}

impl Default for ProceduralSkyData {
    fn default() -> Self {
        Self {
            sky_top_color: (0.15, 0.35, 0.65),       // Deep blue
            sky_horizon_color: (0.55, 0.70, 0.85),   // Light blue
            ground_bottom_color: (0.2, 0.17, 0.13),  // Brown/dirt
            ground_horizon_color: (0.55, 0.55, 0.52), // Light gray
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

/// Procedural clouds settings
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct CloudsData {
    pub enabled: bool,
    /// Cloud coverage (0 = clear, 1 = overcast)
    pub coverage: f32,
    /// Cloud density/opacity (0 = transparent, 1 = opaque)
    pub density: f32,
    /// Noise scale (larger = bigger cloud formations)
    pub scale: f32,
    /// Wind animation speed
    pub speed: f32,
    /// Wind direction in degrees (0-360)
    pub wind_direction: f32,
    /// Altitude threshold (0 = horizon, 1 = zenith)
    pub altitude: f32,
    /// Cloud color (lit side) RGB
    pub color: (f32, f32, f32),
    /// Shadow color (dark underside) RGB
    pub shadow_color: (f32, f32, f32),
}

impl Default for CloudsData {
    fn default() -> Self {
        Self {
            enabled: true,
            coverage: 0.5,
            density: 0.8,
            scale: 4.0,
            speed: 0.02,
            wind_direction: 45.0,
            altitude: 0.3,
            color: (1.0, 1.0, 1.0),
            shadow_color: (0.6, 0.65, 0.7),
        }
    }
}

/// World environment configuration — ambient light only.
/// Sky, fog, and post-processing settings have been moved to individual
/// components (SkyboxData, FogData, BloomData, etc.).
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WorldEnvironmentData {
    pub ambient_color: (f32, f32, f32),
    pub ambient_brightness: f32,
}

impl Default for WorldEnvironmentData {
    fn default() -> Self {
        Self {
            ambient_color: (1.0, 1.0, 1.0),
            ambient_brightness: 300.0,
        }
    }
}
