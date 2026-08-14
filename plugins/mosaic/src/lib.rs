#![no_std]
//! Mosaic post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_mosaic`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("mosaic.wgsl");

#[derive(Component)]
#[component(name = "Mosaic")]
#[repr(C)]
pub struct Mosaic {
    #[field(min = 4.0, max = 200.0, speed = 0.5)]
    pub tile_size: f32,
    #[field(min = 0.0, max = 0.5, speed = 0.01)]
    pub edge_thickness: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub roundness: f32,
}

impl Default for Mosaic {
    fn default() -> Self {
        Self {
            tile_size: 40.0,
            edge_thickness: 0.05,
            roundness: 0.3,
        }
    }
}

pub struct MosaicPlugin;

impl Plugin for MosaicPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Mosaic>("mosaic", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(MosaicPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Mosaic>(WGSL, "MosaicSettings");
    }
}
