//! Heat Haze post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_heat_haze`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("heat_haze.wgsl");

#[derive(Component)]
#[component(name = "Heat Haze")]
#[repr(C)]
pub struct HeatHaze {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.1, max = 10.0, speed = 0.1)]
    pub speed: f32,
    #[field(min = 1.0, max = 100.0, speed = 0.1)]
    pub scale: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for HeatHaze {
    fn default() -> Self {
        Self {
            intensity: 0.15,
            speed: 2.0,
            scale: 20.0,
            time: 0.0,
        }
    }
}

pub struct HeatHazePlugin;

impl Plugin for HeatHazePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<HeatHaze>("heat_haze", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(HeatHazePlugin);
