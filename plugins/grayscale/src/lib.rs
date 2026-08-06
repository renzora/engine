#![no_std]
//! Grayscale post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_grayscale`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("grayscale.wgsl");

#[derive(Component)]
#[component(name = "Grayscale")]
#[repr(C)]
pub struct Grayscale {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(skip)]
    pub luminance_r: f32,
    #[field(skip)]
    pub luminance_g: f32,
    #[field(skip)]
    pub luminance_b: f32,
}

impl Default for Grayscale {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            luminance_r: 0.2126,
            luminance_g: 0.7152,
            luminance_b: 0.0722,
        }
    }
}

pub struct GrayscalePlugin;

impl Plugin for GrayscalePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Grayscale>("grayscale", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GrayscalePlugin);
