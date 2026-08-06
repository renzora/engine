#![no_std]
//! Kaleidoscope post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_kaleidoscope`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("kaleidoscope.wgsl");

#[derive(Component)]
#[component(name = "Kaleidoscope")]
#[repr(C)]
pub struct Kaleidoscope {
    #[field(min = 2.0, max = 32.0, speed = 0.1)]
    pub segments: f32,
    #[field(min = 0.0, max = 6.283, speed = 0.01)]
    pub rotation: f32,
    #[field(skip)]
    pub center_x: f32,
    #[field(skip)]
    pub center_y: f32,
}

impl Default for Kaleidoscope {
    fn default() -> Self {
        Self {
            segments: 6.0,
            rotation: 0.0,
            center_x: 0.5,
            center_y: 0.5,
        }
    }
}

pub struct KaleidoscopePlugin;

impl Plugin for KaleidoscopePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Kaleidoscope>("kaleidoscope", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(KaleidoscopePlugin);
