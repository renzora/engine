#![no_std]
//! Matrix Rain post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_matrix`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("matrix.wgsl");

#[derive(Component)]
#[component(name = "Matrix Rain")]
#[repr(C)]
pub struct Matrix {
    #[field(min = 0.1, max = 10.0, speed = 0.05)]
    pub speed: f32,
    #[field(min = 5.0, max = 50.0, speed = 0.5)]
    pub density: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub glow: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub trail_length: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self {
            speed: 2.0,
            density: 20.0,
            glow: 0.5,
            trail_length: 0.8,
            color_r: 0.0,
            color_g: 1.0,
            time: 0.0,
        }
    }
}

pub struct MatrixPlugin;

impl Plugin for MatrixPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Matrix>("matrix", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(MatrixPlugin);
