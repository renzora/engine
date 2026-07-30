//! Hex Pixelate post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_hex_pixelate`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("hex_pixelate.wgsl");

#[derive(Component)]
#[component(name = "Hex Pixelate")]
#[repr(C)]
pub struct HexPixelate {
    #[field(min = 2.0, max = 50.0, speed = 0.5)]
    pub hex_size: f32,
}

impl Default for HexPixelate {
    fn default() -> Self {
        Self {
            hex_size: 10.0,
        }
    }
}

pub struct HexPixelatePlugin;

impl Plugin for HexPixelatePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<HexPixelate>("hex_pixelate", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(HexPixelatePlugin);
