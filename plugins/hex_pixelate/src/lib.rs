#![no_std]
//! Hex Pixelate post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_hex_pixelate`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("hex_pixelate.wgsl");

#[derive(Component)]
#[component(name = "Hex Pixelate")]
#[repr(C)]
pub struct HexPixelate {
    #[field(min = 2.0, max = 50.0, speed = 0.5)]
    pub hex_size: f32,
}

impl Default for HexPixelate {
    fn default() -> Self {
        Self {
            hex_size: 10.0,
        }
    }
}

pub struct HexPixelatePlugin;

impl Plugin for HexPixelatePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<HexPixelate>("hex_pixelate", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(HexPixelatePlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<HexPixelate>(WGSL, "HexPixelateSettings");
    }
}
