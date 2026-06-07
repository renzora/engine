use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Water preset types for quick configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum WaterPreset {
    CalmLake,
    River,
    Ocean,
    StormyOcean,
    Tropical,
    Arctic,
    Swamp,
}

/// Single Gerstner wave definition.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct GerstnerWave {
    /// Normalized XZ direction of wave travel.
    pub direction: Vec2,
    /// Steepness (Q parameter), 0.0 = sine wave, 1.0 = sharp crests.
    pub steepness: f32,
    /// Distance between wave crests in world units.
    pub wavelength: f32,
    /// Wave height (half the crest-to-trough distance).
    pub amplitude: f32,
}

impl Default for GerstnerWave {
    fn default() -> Self {
        Self {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 10.0,
            amplitude: 0.3,
        }
    }
}

/// Marker component for entities that create wave interactions on water surfaces.
/// Add this to any entity (with a `GlobalTransform`) to generate ripples, foam,
/// and shadow effects where it touches the water — without needing physics/buoyancy.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct WaterInteractor {
    /// Interaction radius (world units). Controls how far ripples spread.
    pub radius: f32,
    /// Interaction intensity (0–1). Controls strength of ripples and foam.
    pub intensity: f32,
}

/// Tag component for water surface entities.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct WaterSurface {
    pub deep_color: [f32; 3],
    pub shallow_color: [f32; 3],
    pub foam_color: [f32; 3],
    pub foam_threshold: f32,
    pub absorption: f32,
    pub roughness: f32,
    pub subsurface_strength: f32,
    pub mesh_size: f32,
    pub waves: Vec<GerstnerWave>,
    pub subdivisions: u32,
    /// Screen-space refraction distortion strength.
    pub refraction_strength: f32,
    /// Maximum absorption depth (world units). Controls how deep objects are visible.
    pub max_depth: f32,
    /// Caustic brightness multiplier.
    pub caustic_intensity: f32,
    /// Sun specular power (higher = tighter highlight).
    pub specular_power: f32,
    /// Absorption coefficient for red channel (higher = absorbed faster).
    pub absorption_r: f32,
    /// Absorption coefficient for green channel.
    pub absorption_g: f32,
    /// Absorption coefficient for blue channel.
    pub absorption_b: f32,
    /// Shoreline foam depth threshold (world units).
    pub foam_depth: f32,
    /// Wind speed (0.0–1.0). Controls wind-driven foam density and detail normal strength.
    pub wind_speed: f32,
    /// Wind direction angle in radians (0–2PI).
    pub wind_angle: f32,
}

impl Default for WaterSurface {
    fn default() -> Self {
        Self {
            deep_color: [0.005, 0.02, 0.08],
            shallow_color: [0.04, 0.22, 0.28],
            foam_color: [0.82, 0.88, 0.92],
            foam_threshold: 0.4,
            absorption: 0.3,
            roughness: 0.15,
            subsurface_strength: 0.3,
            mesh_size: 200.0,
            subdivisions: 256,
            refraction_strength: 0.03,
            max_depth: 8.0,
            caustic_intensity: 0.2,
            specular_power: 3000.0,
            absorption_r: 3.0,
            absorption_g: 1.0,
            absorption_b: 0.4,
            foam_depth: 1.0,
            wind_speed: 0.3,
            wind_angle: 0.0,
            waves: vec![
                // Primary swell
                GerstnerWave {
                    direction: Vec2::new(0.8, 0.6).normalize(),
                    steepness: 0.4,
                    wavelength: 30.0,
                    amplitude: 1.2,
                },
                // Secondary swell (cross-direction)
                GerstnerWave {
                    direction: Vec2::new(-0.3, 0.95).normalize(),
                    steepness: 0.35,
                    wavelength: 20.0,
                    amplitude: 0.7,
                },
                // Medium chop
                GerstnerWave {
                    direction: Vec2::new(0.5, -0.5).normalize(),
                    steepness: 0.5,
                    wavelength: 10.0,
                    amplitude: 0.35,
                },
                // Cross chop
                GerstnerWave {
                    direction: Vec2::new(-0.7, -0.3).normalize(),
                    steepness: 0.45,
                    wavelength: 7.0,
                    amplitude: 0.2,
                },
                // Fine ripple 1
                GerstnerWave {
                    direction: Vec2::new(0.2, -0.9).normalize(),
                    steepness: 0.5,
                    wavelength: 4.0,
                    amplitude: 0.1,
                },
                // Fine ripple 2
                GerstnerWave {
                    direction: Vec2::new(-0.9, 0.4).normalize(),
                    steepness: 0.4,
                    wavelength: 3.0,
                    amplitude: 0.06,
                },
            ],
        }
    }
}

impl WaterSurface {
    /// Apply a preset, overwriting all parameters.
    pub fn from_preset(preset: WaterPreset) -> Self {
        match preset {
            WaterPreset::CalmLake => Self {
                deep_color: [0.01, 0.04, 0.12],
                shallow_color: [0.05, 0.18, 0.22],
                foam_color: [0.85, 0.9, 0.93],
                foam_threshold: 1.5,
                absorption: 0.2,
                roughness: 0.08,
                subsurface_strength: 0.4,
                mesh_size: 200.0,
                subdivisions: 256,
                refraction_strength: 0.04,
                max_depth: 10.0,
                caustic_intensity: 0.3,
                specular_power: 4000.0,
                absorption_r: 2.5,
                absorption_g: 0.8,
                absorption_b: 0.3,
                foam_depth: 0.5,
                wind_speed: 0.05,
                wind_angle: 0.0,
                waves: vec![
                    GerstnerWave {
                        direction: Vec2::new(1.0, 0.3).normalize(),
                        steepness: 0.2,
                        wavelength: 15.0,
                        amplitude: 0.15,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.4, 0.9).normalize(),
                        steepness: 0.15,
                        wavelength: 10.0,
                        amplitude: 0.08,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.6, -0.7).normalize(),
                        steepness: 0.25,
                        wavelength: 5.0,
                        amplitude: 0.04,
                    },
                ],
            },
            WaterPreset::River => Self {
                deep_color: [0.01, 0.05, 0.08],
                shallow_color: [0.06, 0.2, 0.18],
                foam_color: [0.8, 0.85, 0.88],
                foam_threshold: 0.6,
                absorption: 0.4,
                roughness: 0.2,
                subsurface_strength: 0.25,
                mesh_size: 200.0,
                subdivisions: 256,
                refraction_strength: 0.02,
                max_depth: 5.0,
                caustic_intensity: 0.15,
                specular_power: 2000.0,
                absorption_r: 3.5,
                absorption_g: 1.5,
                absorption_b: 0.6,
                foam_depth: 1.5,
                wind_speed: 0.1,
                wind_angle: 0.0,
                waves: vec![
                    // Strong directional flow
                    GerstnerWave {
                        direction: Vec2::new(1.0, 0.0),
                        steepness: 0.3,
                        wavelength: 8.0,
                        amplitude: 0.25,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.9, 0.2).normalize(),
                        steepness: 0.35,
                        wavelength: 5.0,
                        amplitude: 0.15,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.8, -0.3).normalize(),
                        steepness: 0.4,
                        wavelength: 3.0,
                        amplitude: 0.1,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.7, 0.4).normalize(),
                        steepness: 0.45,
                        wavelength: 2.0,
                        amplitude: 0.05,
                    },
                ],
            },
            WaterPreset::Ocean => Self::default(),
            WaterPreset::StormyOcean => Self {
                deep_color: [0.003, 0.012, 0.04],
                shallow_color: [0.02, 0.1, 0.12],
                foam_color: [0.75, 0.8, 0.85],
                foam_threshold: 0.5,
                absorption: 0.5,
                roughness: 0.3,
                subsurface_strength: 0.15,
                mesh_size: 200.0,
                subdivisions: 256,
                refraction_strength: 0.01,
                max_depth: 12.0,
                caustic_intensity: 0.05,
                specular_power: 1000.0,
                absorption_r: 4.0,
                absorption_g: 1.5,
                absorption_b: 0.5,
                foam_depth: 2.0,
                wind_speed: 0.9,
                wind_angle: 0.5,
                waves: vec![
                    // Massive swells
                    GerstnerWave {
                        direction: Vec2::new(0.7, 0.7).normalize(),
                        steepness: 0.5,
                        wavelength: 50.0,
                        amplitude: 3.0,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.4, 0.9).normalize(),
                        steepness: 0.45,
                        wavelength: 35.0,
                        amplitude: 2.0,
                    },
                    // Heavy chop
                    GerstnerWave {
                        direction: Vec2::new(0.9, -0.3).normalize(),
                        steepness: 0.55,
                        wavelength: 15.0,
                        amplitude: 1.0,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.6, -0.5).normalize(),
                        steepness: 0.5,
                        wavelength: 10.0,
                        amplitude: 0.6,
                    },
                    // Aggressive ripples
                    GerstnerWave {
                        direction: Vec2::new(0.3, -0.9).normalize(),
                        steepness: 0.6,
                        wavelength: 5.0,
                        amplitude: 0.3,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.8, 0.2).normalize(),
                        steepness: 0.55,
                        wavelength: 3.5,
                        amplitude: 0.15,
                    },
                ],
            },
            WaterPreset::Tropical => Self {
                deep_color: [0.005, 0.06, 0.15],
                shallow_color: [0.1, 0.45, 0.4],
                foam_color: [0.92, 0.95, 0.97],
                foam_threshold: 1.2,
                absorption: 0.12,
                roughness: 0.06,
                subsurface_strength: 0.5,
                mesh_size: 200.0,
                subdivisions: 256,
                refraction_strength: 0.05,
                max_depth: 15.0,
                caustic_intensity: 0.5,
                specular_power: 5000.0,
                absorption_r: 1.5,
                absorption_g: 0.3,
                absorption_b: 0.1,
                foam_depth: 0.8,
                wind_speed: 0.05,
                wind_angle: 0.0,
                waves: vec![
                    GerstnerWave {
                        direction: Vec2::new(0.8, 0.5).normalize(),
                        steepness: 0.2,
                        wavelength: 20.0,
                        amplitude: 0.3,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.3, 0.9).normalize(),
                        steepness: 0.15,
                        wavelength: 12.0,
                        amplitude: 0.15,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.5, -0.6).normalize(),
                        steepness: 0.25,
                        wavelength: 6.0,
                        amplitude: 0.06,
                    },
                ],
            },
            WaterPreset::Arctic => Self {
                deep_color: [0.008, 0.02, 0.05],
                shallow_color: [0.08, 0.15, 0.2],
                foam_color: [0.85, 0.88, 0.92],
                foam_threshold: 0.6,
                absorption: 0.35,
                roughness: 0.12,
                subsurface_strength: 0.2,
                mesh_size: 200.0,
                subdivisions: 256,
                refraction_strength: 0.02,
                max_depth: 8.0,
                caustic_intensity: 0.1,
                specular_power: 2500.0,
                absorption_r: 3.0,
                absorption_g: 1.2,
                absorption_b: 0.5,
                foam_depth: 1.0,
                wind_speed: 0.4,
                wind_angle: 1.2,
                waves: vec![
                    GerstnerWave {
                        direction: Vec2::new(0.6, 0.8).normalize(),
                        steepness: 0.35,
                        wavelength: 25.0,
                        amplitude: 1.0,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.5, 0.7).normalize(),
                        steepness: 0.3,
                        wavelength: 15.0,
                        amplitude: 0.5,
                    },
                    GerstnerWave {
                        direction: Vec2::new(0.8, -0.4).normalize(),
                        steepness: 0.4,
                        wavelength: 8.0,
                        amplitude: 0.25,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.3, -0.9).normalize(),
                        steepness: 0.45,
                        wavelength: 5.0,
                        amplitude: 0.12,
                    },
                ],
            },
            WaterPreset::Swamp => Self {
                deep_color: [0.01, 0.02, 0.005],
                shallow_color: [0.04, 0.08, 0.03],
                foam_color: [0.3, 0.35, 0.2],
                foam_threshold: 2.0,
                absorption: 0.8,
                roughness: 0.4,
                subsurface_strength: 0.05,
                mesh_size: 200.0,
                subdivisions: 128,
                refraction_strength: 0.005,
                max_depth: 3.0,
                caustic_intensity: 0.0,
                specular_power: 500.0,
                absorption_r: 5.0,
                absorption_g: 3.0,
                absorption_b: 2.0,
                foam_depth: 0.3,
                wind_speed: 0.0,
                wind_angle: 0.0,
                waves: vec![
                    GerstnerWave {
                        direction: Vec2::new(1.0, 0.2).normalize(),
                        steepness: 0.1,
                        wavelength: 8.0,
                        amplitude: 0.03,
                    },
                    GerstnerWave {
                        direction: Vec2::new(-0.3, 0.8).normalize(),
                        steepness: 0.08,
                        wavelength: 5.0,
                        amplitude: 0.02,
                    },
                ],
            },
        }
    }
}

// Inspector entries for `WaterSurface` / `WaterInteractor` are editor-only and
// live in the `renzora_water_editor` crate (crates/renzora_water/editor). The
// data types above stay `pub` so that crate can read/write them.
