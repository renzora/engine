#![no_std]
//! Film Grain post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_film_grain`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("film_grain.wgsl");

#[derive(Component)]
#[component(name = "Film Grain")]
#[repr(C)]
pub struct FilmGrain {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.1, max = 10.0, speed = 0.1)]
    pub grain_size: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for FilmGrain {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            grain_size: 1.5,
            time: 0.0,
        }
    }
}

pub struct FilmGrainPlugin;

impl Plugin for FilmGrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<FilmGrain>("film_grain", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(FilmGrainPlugin);
