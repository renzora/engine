#![no_std]
//! God Rays post-process effect.
//!
//! Converted from `crates/renzora_god_rays`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("god_rays.wgsl");

#[derive(Component)]
#[component(name = "God Rays")]
#[repr(C)]
pub struct GodRays {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.9, max = 1.0, speed = 0.001)]
    pub decay: f32,
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub density: f32,
    /// Ray-march step count. Not inspectable — `FieldKind` has no `u32` — but it
    /// MUST sit here, between `density` and `light_pos_x`, because that is where
    /// the uniform block has it. While it was missing, the shader read the light
    /// position as the sample count and `light_pos_y` as `light_pos_x`, so the
    /// rays streamed from the wrong place.
    #[field(skip)]
    pub num_samples: u32,
    #[field(min = -1.0, max = 2.0, speed = 0.01)]
    pub light_pos_x: f32,
    #[field(min = -1.0, max = 2.0, speed = 0.01)]
    pub light_pos_y: f32,
}

impl Default for GodRays {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            decay: 0.97,
            density: 1.0,
            // The shader clamps to 128; 64 is the usual god-rays step count.
            num_samples: 64,
            light_pos_x: 0.5,
            light_pos_y: 0.3,
        }
    }
}

pub struct GodRaysPlugin;

impl Plugin for GodRaysPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<GodRays>("god_rays", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GodRaysPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<GodRays>(WGSL, "GodRaysSettings");
    }
}
