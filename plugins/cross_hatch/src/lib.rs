#![no_std]
//! Cross Hatch post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_cross_hatch`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("cross_hatch.wgsl");

#[derive(Component)]
#[component(name = "Cross Hatch")]
#[repr(C)]
pub struct CrossHatch {
    #[field(min = 2.0, max = 100.0, speed = 0.5)]
    pub density: f32,
    #[field(min = 0.01, max = 0.5, speed = 0.01)]
    pub thickness: f32,
    #[field(min = 0.0, max = 1.57, speed = 0.01)]
    pub angle: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub brightness: f32,
}

impl Default for CrossHatch {
    fn default() -> Self {
        Self {
            density: 30.0,
            thickness: 0.1,
            angle: 0.785,
            brightness: 0.9,
        }
    }
}

pub struct CrossHatchPlugin;

impl Plugin for CrossHatchPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<CrossHatch>("cross_hatch", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(CrossHatchPlugin);
