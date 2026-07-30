//! Halftone post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_halftone`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("halftone.wgsl");

#[derive(Component)]
#[component(name = "Halftone")]
#[repr(C)]
pub struct Halftone {
    #[field(min = 2.0, max = 20.0, speed = 0.1)]
    pub dot_size: f32,
    #[field(min = 0.0, max = 3.14159, speed = 0.01)]
    pub angle: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
}

impl Default for Halftone {
    fn default() -> Self {
        Self {
            dot_size: 4.0,
            angle: 0.785,
            intensity: 1.0,
        }
    }
}

pub struct HalftonePlugin;

impl Plugin for HalftonePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Halftone>("halftone", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(HalftonePlugin);
