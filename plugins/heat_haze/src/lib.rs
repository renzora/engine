#![no_std]
//! Heat Haze post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_heat_haze`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("heat_haze.wgsl");

#[derive(Component)]
#[component(name = "Heat Haze")]
#[repr(C)]
pub struct HeatHaze {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.1, max = 10.0, speed = 0.1)]
    pub speed: f32,
    #[field(min = 1.0, max = 100.0, speed = 0.1)]
    pub scale: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for HeatHaze {
    fn default() -> Self {
        Self {
            intensity: 0.15,
            speed: 2.0,
            scale: 20.0,
            time: 0.0,
        }
    }
}

pub struct HeatHazePlugin;

impl Plugin for HeatHazePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<HeatHaze>("heat_haze", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(HeatHazePlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust struct and the shader must agree byte for byte. Nothing enforces
    /// it at run time — the host copies these bytes straight into the uniform
    /// buffer and the shader reads them back by offset — so a mismatch is not an
    /// error, it is a wrong picture: every field from the mismatch onward reads
    /// its neighbour's value.
    #[test]
    fn the_uniform_matches_the_shader() {
        renzora_plugin::uniform_check::assert_uniform_matches::<HeatHaze>(WGSL, "HeatHazeSettings");
    }
}
