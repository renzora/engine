#![no_std]
//! Light Streaks post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_light_streaks`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("light_streaks.wgsl");

#[derive(Component)]
#[component(name = "Light Streaks")]
#[repr(C)]
pub struct LightStreaks {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub threshold: f32,
    #[field(min = 4.0, max = 32.0, speed = 1.0)]
    pub samples: f32,
    #[field(min = 0.0, max = 6.283, speed = 0.01)]
    pub direction: f32,
}

impl Default for LightStreaks {
    fn default() -> Self {
        Self {
            intensity: 0.4,
            threshold: 0.7,
            samples: 12.0,
            direction: 0.0,
        }
    }
}

pub struct LightStreaksPlugin;

impl Plugin for LightStreaksPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<LightStreaks>("light_streaks", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(LightStreaksPlugin);
