#![no_std]
//! Wave post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_wave`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("wave.wgsl");

#[derive(Component)]
#[component(name = "Wave")]
#[repr(C)]
pub struct Wave {
    #[field(min = 0.0, max = 0.1, speed = 0.001)]
    pub amplitude: f32,
    #[field(min = 1.0, max = 50.0, speed = 0.5)]
    pub frequency: f32,
    #[field(min = 0.1, max = 10.0, speed = 0.1)]
    pub speed: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            amplitude: 0.01,
            frequency: 10.0,
            speed: 2.0,
            time: 0.0,
        }
    }
}

pub struct WavePlugin;

impl Plugin for WavePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Wave>("wave", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(WavePlugin);
