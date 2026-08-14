#![no_std]
//! Distortion post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_distortion`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("distortion.wgsl");

#[derive(Component)]
#[component(name = "Distortion")]
#[repr(C)]
pub struct Distortion {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.0, max = 10.0, speed = 0.01)]
    pub speed: f32,
    #[field(min = 0.1, max = 50.0, speed = 0.1)]
    pub scale: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Distortion {
    fn default() -> Self {
        Self {
            intensity: 0.02,
            speed: 1.0,
            scale: 10.0,
            time: 0.0,
        }
    }
}

pub struct DistortionPlugin;

impl Plugin for DistortionPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Distortion>("distortion", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(DistortionPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Distortion>(WGSL, "DistortionSettings");
    }
}
