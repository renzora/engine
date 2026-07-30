//! Oil Painting post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_oil_painting`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("oil_painting.wgsl");

#[derive(Component)]
#[component(name = "Oil Painting")]
#[repr(C)]
pub struct OilPainting {
    #[field(min = 1.0, max = 8.0, speed = 0.1)]
    pub radius: f32,
    #[field(min = 4.0, max = 32.0, speed = 0.5)]
    pub levels: f32,
}

impl Default for OilPainting {
    fn default() -> Self {
        Self {
            radius: 3.0,
            levels: 8.0,
        }
    }
}

pub struct OilPaintingPlugin;

impl Plugin for OilPaintingPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<OilPainting>("oil_painting", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(OilPaintingPlugin);
