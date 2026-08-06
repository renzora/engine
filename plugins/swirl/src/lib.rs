#![no_std]
//! Swirl post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_swirl`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("swirl.wgsl");

#[derive(Component)]
#[component(name = "Swirl")]
#[repr(C)]
pub struct Swirl {
    #[field(min = -10.0, max = 10.0, speed = 0.01)]
    pub angle: f32,
    #[field(min = 0.01, max = 2.0, speed = 0.01)]
    pub radius: f32,
    #[field(skip)]
    pub center_x: f32,
    #[field(skip)]
    pub center_y: f32,
}

impl Default for Swirl {
    fn default() -> Self {
        Self {
            angle: 3.0,
            radius: 0.5,
            center_x: 0.5,
            center_y: 0.5,
        }
    }
}

pub struct SwirlPlugin;

impl Plugin for SwirlPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Swirl>("swirl", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SwirlPlugin);
