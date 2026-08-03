//! Glitch post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_glitch`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("glitch.wgsl");

#[derive(Component)]
#[component(name = "Glitch")]
#[repr(C)]
pub struct Glitch {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 4.0, max = 64.0, speed = 1.0)]
    pub block_size: f32,
    #[field(min = 0.0, max = 0.1, speed = 0.001)]
    pub color_drift: f32,
    #[field(min = 0.1, max = 20.0, speed = 0.1)]
    pub speed: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Glitch {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            block_size: 16.0,
            color_drift: 0.01,
            speed: 5.0,
            time: 0.0,
        }
    }
}

pub struct GlitchPlugin;

impl Plugin for GlitchPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Glitch>("glitch", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GlitchPlugin);
