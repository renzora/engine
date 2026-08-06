#![no_std]
//! Frosted Glass post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_frosted_glass`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("frosted_glass.wgsl");

#[derive(Component)]
#[component(name = "Frosted Glass")]
#[repr(C)]
pub struct FrostedGlass {
    #[field(min = 0.0, max = 0.05, speed = 0.001)]
    pub intensity: f32,
    #[field(min = 1.0, max = 50.0, speed = 0.5)]
    pub scale: f32,
}

impl Default for FrostedGlass {
    fn default() -> Self {
        Self {
            intensity: 0.01,
            scale: 10.0,
        }
    }
}

pub struct FrostedGlassPlugin;

impl Plugin for FrostedGlassPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<FrostedGlass>("frosted_glass", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(FrostedGlassPlugin);
