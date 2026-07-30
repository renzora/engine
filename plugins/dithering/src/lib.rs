//! Dithering post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_dithering`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("dithering.wgsl");

#[derive(Component)]
#[component(name = "Dithering")]
#[repr(C)]
pub struct Dithering {
    #[field(min = 2.0, max = 32.0, speed = 0.5)]
    pub color_depth: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
}

impl Default for Dithering {
    fn default() -> Self {
        Self {
            color_depth: 8.0,
            intensity: 1.0,
        }
    }
}

pub struct DitheringPlugin;

impl Plugin for DitheringPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Dithering>("dithering", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(DitheringPlugin);
