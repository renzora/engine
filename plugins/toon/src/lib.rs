#![no_std]
//! Toon post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_toon`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("toon.wgsl");

#[derive(Component)]
#[component(name = "Toon")]
#[repr(C)]
pub struct Toon {
    #[field(min = 2.0, max = 16.0, speed = 0.1)]
    pub levels: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.005)]
    pub edge_threshold: f32,
    #[field(min = 0.5, max = 5.0, speed = 0.05)]
    pub edge_thickness: f32,
    #[field(min = 0.0, max = 3.0, speed = 0.02)]
    pub saturation_boost: f32,
}

impl Default for Toon {
    fn default() -> Self {
        Self {
            levels: 4.0,
            edge_threshold: 0.1,
            edge_thickness: 1.0,
            saturation_boost: 1.2,
        }
    }
}

pub struct ToonPlugin;

impl Plugin for ToonPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Toon>("toon", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ToonPlugin);
