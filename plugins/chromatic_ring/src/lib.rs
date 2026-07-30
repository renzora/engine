//! Chromatic Ring post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_chromatic_ring`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("chromatic_ring.wgsl");

#[derive(Component)]
#[component(name = "Chromatic Ring")]
#[repr(C)]
pub struct ChromaticRing {
    #[field(min = 0.0, max = 0.05, speed = 0.001)]
    pub intensity: f32,
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub radius: f32,
    #[field(min = 0.01, max = 1.0, speed = 0.01)]
    pub falloff: f32,
}

impl Default for ChromaticRing {
    fn default() -> Self {
        Self {
            intensity: 0.008,
            radius: 0.8,
            falloff: 0.4,
        }
    }
}

pub struct ChromaticRingPlugin;

impl Plugin for ChromaticRingPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<ChromaticRing>("chromatic_ring", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ChromaticRingPlugin);
