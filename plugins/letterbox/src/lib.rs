#![no_std]
//! Letterbox post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_letterbox`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("letterbox.wgsl");

#[derive(Component)]
#[component(name = "Letterbox")]
#[repr(C)]
pub struct Letterbox {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub bar_height: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub softness: f32,
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub aspect_ratio: f32,
}

impl Default for Letterbox {
    fn default() -> Self {
        Self {
            bar_height: 0.12,
            softness: 0.0,
            aspect_ratio: 0.0,
        }
    }
}

pub struct LetterboxPlugin;

impl Plugin for LetterboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Letterbox>("letterbox", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(LetterboxPlugin);
