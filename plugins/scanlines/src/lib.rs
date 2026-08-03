//! Scanlines post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_scanlines`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("scanlines.wgsl");

#[derive(Component)]
#[component(name = "Scanlines")]
#[repr(C)]
pub struct Scanlines {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 10.0, max = 2000.0, speed = 10.0)]
    pub count: f32,
    #[field(min = 0.0, max = 10.0, speed = 0.1)]
    pub speed: f32,
}

impl Default for Scanlines {
    fn default() -> Self {
        Self {
            intensity: 0.15,
            count: 800.0,
            speed: 0.0,
        }
    }
}

pub struct ScanlinesPlugin;

impl Plugin for ScanlinesPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Scanlines>("scanlines", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ScanlinesPlugin);
