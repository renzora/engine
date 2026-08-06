#![no_std]
//! Invert post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_invert`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("invert.wgsl");

#[derive(Component)]
#[component(name = "Invert")]
#[repr(C)]
pub struct Invert {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
}

impl Default for Invert {
    fn default() -> Self {
        Self {
            intensity: 1.0,
        }
    }
}

pub struct InvertPlugin;

impl Plugin for InvertPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Invert>("invert", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(InvertPlugin);
