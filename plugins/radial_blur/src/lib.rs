#![no_std]
//! Radial Blur post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_radial_blur`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("radial_blur.wgsl");

#[derive(Component)]
#[component(name = "Radial Blur")]
#[repr(C)]
pub struct RadialBlur {
    #[field(min = 0.0, max = 0.2, speed = 0.001)]
    pub intensity: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub center_x: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub center_y: f32,
    #[field(min = 4.0, max = 32.0, speed = 1.0)]
    pub samples: f32,
}

impl Default for RadialBlur {
    fn default() -> Self {
        Self {
            intensity: 0.02,
            center_x: 0.5,
            center_y: 0.5,
            samples: 8.0,
        }
    }
}

pub struct RadialBlurPlugin;

impl Plugin for RadialBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<RadialBlur>("radial_blur", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(RadialBlurPlugin);
