#![no_std]
//! Outline post-process effect.
//!
//! Converted from `crates/renzora_outline`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("outline.wgsl");

#[derive(Component)]
#[component(name = "Outline")]
#[repr(C)]
pub struct Outline {
    #[field(min = 0.5, max = 5.0, speed = 0.05)]
    pub thickness: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.005)]
    pub threshold: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub color_b: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub mix_mode: f32,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            thickness: 1.0,
            threshold: 0.1,
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
            mix_mode: 0.0,
        }
    }
}

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Outline>("outline", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(OutlinePlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<Outline>(WGSL, "OutlineSettings");
    }
}
