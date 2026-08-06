#![no_std]
//! Vibrance post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_vibrance`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("vibrance.wgsl");

#[derive(Component)]
#[component(name = "Vibrance")]
#[repr(C)]
pub struct Vibrance {
    #[field(min = -1.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
}

impl Default for Vibrance {
    fn default() -> Self {
        Self {
            intensity: 0.5,
        }
    }
}

pub struct VibrancePlugin;

impl Plugin for VibrancePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Vibrance>("vibrance", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(VibrancePlugin);
