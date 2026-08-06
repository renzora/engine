#![no_std]
//! Pixelation post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_pixelation`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("pixelation.wgsl");

#[derive(Component)]
#[component(name = "Pixelation")]
#[repr(C)]
pub struct Pixelation {
    #[field(min = 1.0, max = 64.0, speed = 0.5)]
    pub pixel_size: f32,
}

impl Default for Pixelation {
    fn default() -> Self {
        Self {
            pixel_size: 4.0,
        }
    }
}

pub struct PixelationPlugin;

impl Plugin for PixelationPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Pixelation>("pixelation", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(PixelationPlugin);
