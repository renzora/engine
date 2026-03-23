use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Night stars — procedural starfield rendered on a sky dome
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct NightStarsData {
    pub enabled: bool,
    /// Star density (0 = very few, 1 = dense starfield)
    pub density: f32,
    /// Brightness multiplier (0..10)
    pub brightness: f32,
    /// Star angular size (0.2 = tiny dots, 5.0 = large blobs)
    pub star_size: f32,
    /// Twinkling animation speed (0 = static, 10 = fast)
    pub twinkle_speed: f32,
    /// Twinkling intensity (0 = no twinkle, 1 = strong)
    pub twinkle_amount: f32,
    /// Elevation angle at which stars fade in above the horizon (0..1)
    pub horizon_fade: f32,
    /// Star color tint (RGB)
    pub color: (f32, f32, f32),
}

impl Default for NightStarsData {
    fn default() -> Self {
        Self {
            enabled: true,
            density: 0.55,
            brightness: 1.5,
            star_size: 1.2,
            twinkle_speed: 1.0,
            twinkle_amount: 0.35,
            horizon_fade: 0.08,
            color: (1.0, 0.97, 0.9), // slightly warm white
        }
    }
}
