#![no_std]
//! Grayscale post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_grayscale`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("grayscale.wgsl");

#[derive(Component)]
#[component(name = "Grayscale")]
#[repr(C)]
pub struct Grayscale {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(skip)]
    pub luminance_r: f32,
    #[field(skip)]
    pub luminance_g: f32,
    #[field(skip)]
    pub luminance_b: f32,
}

impl Default for Grayscale {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            luminance_r: 0.2126,
            luminance_g: 0.7152,
            luminance_b: 0.0722,
        }
    }
}

pub struct GrayscalePlugin;

impl Plugin for GrayscalePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Grayscale>("grayscale", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GrayscalePlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Grayscale>(WGSL, "GrayscaleSettings");
    }
}
