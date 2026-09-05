//! `AutoExposureSettings` — the authored auto-exposure settings.
//!
//! The metering and curve-building live in the `auto_exposure` **native
//! plugin**; only the settings type is here, for the same reason as
//! [`crate::clouds`]: `renzora_level_presets` inserts and queries this component
//! while compiled into the editor binary, and the `debugger` plugin reads it to
//! show the live EV — both of them binaries that cannot name a type living in a
//! plugin. One definition here means one `TypeId` on both sides of the dlopen
//! boundary.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AutoExposureSettings {
    /// How fast the camera adapts to brighter scenes, in F-stops/second.
    /// Bevy's default is 3.0 (eye adapts to bright quickly).
    pub speed_brighten: f32,
    /// How fast the camera adapts to darker scenes, in F-stops/second.
    /// Bevy's default is 1.0 (eye adapts to dark slowly).
    pub speed_darken: f32,
    /// Minimum EV the metering can drive towards. Bevy default: -8.
    pub range_min: f32,
    /// Maximum EV the metering can drive towards. Bevy default: +8.
    pub range_max: f32,
    /// Lower percentile cutoff (0..1). Pixels darker than this fraction
    /// of the histogram are excluded from metering. 0.10 = ignore the
    /// darkest 10%. This is what stops a dark/empty scene from pulling
    /// the average toward zero and blowing the frame to white.
    pub filter_low: f32,
    /// Upper percentile cutoff. 0.90 = ignore brightest 10% (specular
    /// highlights, sun disk, etc.).
    pub filter_high: f32,
    /// Anti-jitter band in F-stops. Small frame-to-frame changes within
    /// this band animate exponentially (slow, smooth); larger changes
    /// use the linear `speed_*` rates. 1.5 = Bevy default.
    pub exponential_transition_distance: f32,
    /// How strongly to keep dark (night) scenes dark instead of letting
    /// auto-exposure lift them to middle gray. `0.0` = pure Bevy AE (a night
    /// scene is brightened — washed out); `1.0` ≈ the metered darkness is
    /// preserved (night stays night). Implemented as the exposure-compensation
    /// curve: flat (no change) at/above `keep_dark_pivot_ev` so daytime is
    /// untouched, ramping negative below it.
    #[serde(default = "default_keep_dark_strength")]
    pub keep_dark_strength: f32,
    /// Metered scene brightness (EV-100, the histogram average) at/above which
    /// NO dark-compensation is applied — daytime stays exactly as Bevy AE
    /// renders it. Below it, compensation ramps in. Raise it if nights still
    /// wash out; lower it if dusk / interiors get too dark.
    #[serde(default = "default_keep_dark_pivot")]
    pub keep_dark_pivot_ev: f32,
    pub enabled: bool,
}

fn default_keep_dark_strength() -> f32 {
    0.7
}
fn default_keep_dark_pivot() -> f32 {
    2.0
}

impl Default for AutoExposureSettings {
    fn default() -> Self {
        // Mirrors Bevy's `AutoExposure::default()` field-for-field —
        // these are the values the Bevy team picked after testing
        // against real scenes.
        Self {
            speed_brighten: 3.0,
            speed_darken: 1.0,
            range_min: -8.0,
            range_max: 8.0,
            filter_low: 0.10,
            filter_high: 0.90,
            exponential_transition_distance: 1.5,
            keep_dark_strength: default_keep_dark_strength(),
            keep_dark_pivot_ev: default_keep_dark_pivot(),
            enabled: true,
        }
    }
}
