#![no_std]
//! Sepia tone mapping, as a standalone C-ABI plugin.
//!
//! The simplest of the three conversions, and the one that shows `#[field(skip)]`:
//! the tone weights are real values the shader reads every pixel, but they are a
//! tuned constant rather than something to put three sliders on. Skipping keeps
//! them in the struct and out of the inspector.
//!
//! See `plugins/crt` for why there is no padding and no `enabled` flag.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("sepia.wgsl");

#[derive(Component)]
#[repr(C)]
pub struct Sepia {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(skip)]
    pub tone_r: f32,
    #[field(skip)]
    pub tone_g: f32,
    #[field(skip)]
    pub tone_b: f32,
}

impl Default for Sepia {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            tone_r: 1.2,
            tone_g: 1.0,
            tone_b: 0.8,
        }
    }
}

pub struct SepiaPlugin;

impl Plugin for SepiaPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Sepia>("sepia", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SepiaPlugin);
