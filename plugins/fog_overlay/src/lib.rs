#![no_std]
//! Fog Overlay post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_fog_overlay`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("fog_overlay.wgsl");

#[derive(Component)]
#[component(name = "Fog Overlay")]
#[repr(C)]
pub struct FogOverlay {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub density: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub height: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub color_b: f32,
}

impl Default for FogOverlay {
    fn default() -> Self {
        Self {
            density: 0.3,
            height: 0.3,
            color_r: 0.7,
            color_g: 0.75,
            color_b: 0.8,
        }
    }
}

pub struct FogOverlayPlugin;

impl Plugin for FogOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<FogOverlay>("fog_overlay", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(FogOverlayPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<FogOverlay>(WGSL, "FogOverlaySettings");
    }
}
