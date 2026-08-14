#![no_std]
//! Edge Glow post-process effect.
//!
//! Converted from `crates/renzora_edge_glow`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("edge_glow.wgsl");

#[derive(Component)]
#[component(name = "Edge Glow")]
#[repr(C)]
pub struct EdgeGlow {
    #[field(min = 0.0, max = 1.0, speed = 0.005)]
    pub threshold: f32,
    #[field(min = 0.0, max = 5.0, speed = 0.05)]
    pub glow_intensity: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub color_b: f32,
}

impl Default for EdgeGlow {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            glow_intensity: 2.0,
            color_r: 0.0,
            color_g: 1.0,
            color_b: 1.0,
        }
    }
}

pub struct EdgeGlowPlugin;

impl Plugin for EdgeGlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<EdgeGlow>("edge_glow", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(EdgeGlowPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<EdgeGlow>(WGSL, "EdgeGlowSettings");
    }
}
