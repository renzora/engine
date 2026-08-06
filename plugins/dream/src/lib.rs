#![no_std]
//! Dream post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_dream`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("dream.wgsl");

#[derive(Component)]
#[component(name = "Dream")]
#[repr(C)]
pub struct Dream {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 1.0, max = 10.0, speed = 0.1)]
    pub blur_radius: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub threshold: f32,
}

impl Default for Dream {
    fn default() -> Self {
        Self {
            intensity: 0.4,
            blur_radius: 3.0,
            threshold: 0.5,
        }
    }
}

pub struct DreamPlugin;

impl Plugin for DreamPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Dream>("dream", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(DreamPlugin);
