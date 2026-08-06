#![no_std]
//! Pillarbox post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_pillowbox`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("pillowbox.wgsl");

#[derive(Component)]
#[component(name = "Pillarbox")]
#[repr(C)]
pub struct Pillowbox {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub bar_width: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub softness: f32,
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub aspect_ratio: f32,
}

impl Default for Pillowbox {
    fn default() -> Self {
        Self {
            bar_width: 0.15,
            softness: 0.0,
            aspect_ratio: 0.0,
        }
    }
}

pub struct PillowboxPlugin;

impl Plugin for PillowboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Pillowbox>("pillowbox", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(PillowboxPlugin);
