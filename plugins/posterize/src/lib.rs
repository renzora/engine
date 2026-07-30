//! Posterize post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_posterize`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("posterize.wgsl");

#[derive(Component)]
#[component(name = "Posterize")]
#[repr(C)]
pub struct Posterize {
    #[field(min = 2.0, max = 64.0, speed = 1.0)]
    pub levels: f32,
}

impl Default for Posterize {
    fn default() -> Self {
        Self {
            levels: 8.0,
        }
    }
}

pub struct PosterizePlugin;

impl Plugin for PosterizePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Posterize>("posterize", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(PosterizePlugin);
