#![no_std]
//! Thermal Vision post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_thermal`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("thermal.wgsl");

#[derive(Component)]
#[component(name = "Thermal Vision")]
#[repr(C)]
pub struct Thermal {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.1, max = 3.0, speed = 0.01)]
    pub contrast: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub cold_threshold: f32,
}

impl Default for Thermal {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            contrast: 1.5,
            cold_threshold: 0.3,
        }
    }
}

pub struct ThermalPlugin;

impl Plugin for ThermalPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Thermal>("thermal", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ThermalPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Thermal>(WGSL, "ThermalSettings");
    }
}
