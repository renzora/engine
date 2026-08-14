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
        renzora_plugin::uniform_check::assert_uniform_matches::<Kaleidoscope>(WGSL, "KaleidoscopeSettings");
    }
}
