#![no_std]
//! Rain post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_rain`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("rain.wgsl");

#[derive(Component)]
#[component(name = "Rain")]
#[repr(C)]
pub struct Rain {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.1, max = 5.0, speed = 0.1)]
    pub speed: f32,
    #[field(min = 1.0, max = 20.0, speed = 0.1)]
    pub drop_size: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Rain {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            speed: 1.0,
            drop_size: 8.0,
            time: 0.0,
        }
    }
}

pub struct RainPlugin;

impl Plugin for RainPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Rain>("rain", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(RainPlugin);
