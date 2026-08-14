#![no_std]
//! ASCII art post-process, as a standalone C-ABI plugin.
//!
//! Three settings, three fields — 12 bytes, which the host rounds to a 16-byte
//! uniform buffer on its own. The old version of this effect padded to 32 by hand;
//! see `plugins/crt` for why that is gone.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("ascii.wgsl");

#[derive(Component)]
#[repr(C)]
pub struct Ascii {
    #[field(min = 2.0, max = 32.0, speed = 0.5)]
    pub char_size: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub color_mix: f32,
    #[field(min = 0.5, max = 3.0, speed = 0.01)]
    pub contrast: f32,
}

impl Default for Ascii {
    fn default() -> Self {
        Self {
            char_size: 8.0,
            color_mix: 0.5,
            contrast: 1.2,
        }
    }
}

pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Ascii>("ascii", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(AsciiPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Ascii>(WGSL, "AsciiSettings");
    }
}
