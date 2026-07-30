//! Color Grading post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_color_grading`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("color_grading.wgsl");

#[derive(Component)]
#[component(name = "Color Grading")]
#[repr(C)]
pub struct ColorGrading {
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub brightness: f32,
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub contrast: f32,
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub saturation: f32,
    #[field(min = 0.1, max = 3.0, speed = 0.01)]
    pub gamma: f32,
    #[field(min = -1.0, max = 1.0, speed = 0.01)]
    pub temperature: f32,
    #[field(min = -1.0, max = 1.0, speed = 0.01)]
    pub tint: f32,
}

impl Default for ColorGrading {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 1.0,
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

pub struct ColorGradingPlugin;

impl Plugin for ColorGradingPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<ColorGrading>("color_grading", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ColorGradingPlugin);
