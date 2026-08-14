#![no_std]
//! CRT screen effect, as a standalone C-ABI plugin.
//!
//! One of the first three effects moved off the Bevy-linking `crates/renzora_crt`,
//! and the template for the rest.
//!
//! ## The struct is just the settings
//!
//! No padding, no `enabled` flag. The old `#[post_process]` macro generated both —
//! every effect's uniform was padded to 32 bytes and carried a float the shader
//! early-outed on — and the first version of this conversion copied that shape
//! over. That was wrong: it made the padding an authoring burden (count slots by
//! hand to reach 32) and capped an effect at seven fields.
//!
//! The host already rounds the uniform buffer to a 16-byte multiple
//! (`plugin_bridge`), so a settings struct can be any size and any number of
//! fields. Write what the effect needs and let the layout follow.
//!
//! Switching an effect is adding or removing the component, which is what
//! `enabled` was standing in for.
//!
//! ## What it buys
//!
//! The plugin links no Bevy: it rebuilds in about a second and hot-reloads, WGSL
//! included — `include_str!` rather than an embedded asset, so editing the shader
//! is a source change the watcher already sees.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("crt.wgsl");

#[derive(Component)]
#[repr(C)]
pub struct Crt {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub scanline_intensity: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub curvature: f32,
    #[field(min = 0.0, max = 0.1, speed = 0.001)]
    pub chromatic_amount: f32,
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub vignette_amount: f32,
}

impl Default for Crt {
    fn default() -> Self {
        Self {
            scanline_intensity: 0.3,
            curvature: 0.02,
            chromatic_amount: 0.003,
            vignette_amount: 0.5,
        }
    }
}

pub struct CrtPlugin;

impl Plugin for CrtPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Crt>("crt", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(CrtPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust struct and the shader must agree byte for byte. Nothing enforces
    /// it at run time — the host copies these bytes straight into the uniform
    /// buffer and the shader reads them back by offset — so a mismatch is a
    /// wrong picture, not an error.
    #[test]
    fn the_uniform_matches_the_shader() {
        renzora_plugin::uniform_check::assert_uniform_matches::<Crt>(WGSL, "CrtSettings");
    }

    /// A default outside its own declared range is silently clamped the first
    /// time the user touches the slider, which changes the effect without them
    /// asking.
    #[test]
    fn defaults_sit_inside_their_declared_ranges() {
        let d = Crt::default();
        assert!((0.0..=2.0).contains(&d.scanline_intensity));
        assert!((0.0..=1.0).contains(&d.curvature));
        assert!((0.0..=0.1).contains(&d.chromatic_amount));
        assert!((0.0..=2.0).contains(&d.vignette_amount));
    }
}
