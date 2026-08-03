//! Chromatic Aberration post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_chromatic_aberration`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("chromatic_aberration.wgsl");

#[derive(Component)]
#[component(name = "Chromatic Aberration")]
#[repr(C)]
pub struct ChromaticAberration {
    #[field(min = 0.0, max = 0.1, speed = 0.001)]
    pub intensity: f32,
    #[field(min = 1.0, max = 16.0, speed = 1.0)]
    pub samples: f32,
    #[field(skip)]
    pub direction_x: f32,
    #[field(skip)]
    pub direction_y: f32,
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self {
            intensity: 0.005,
            samples: 3.0,
            direction_x: 1.0,
            direction_y: 0.0,
        }
    }
}

pub struct ChromaticAberrationPlugin;

impl Plugin for ChromaticAberrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<ChromaticAberration>("chromatic_aberration", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ChromaticAberrationPlugin);
