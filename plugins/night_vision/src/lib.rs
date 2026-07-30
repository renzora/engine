//! Night Vision post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_night_vision`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("night_vision.wgsl");

#[derive(Component)]
#[component(name = "Night Vision")]
#[repr(C)]
pub struct NightVision {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub noise_amount: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub scanline_amount: f32,
    #[field(min = 1.0, max = 10.0, speed = 0.05)]
    pub color_amplification: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for NightVision {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            noise_amount: 0.15,
            scanline_amount: 0.3,
            color_amplification: 3.0,
            time: 0.0,
        }
    }
}

pub struct NightVisionPlugin;

impl Plugin for NightVisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<NightVision>("night_vision", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(NightVisionPlugin);
