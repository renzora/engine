#![no_std]
//! Gaussian Blur post-process effect.
//!
//! Converted from `crates/renzora_gaussian_blur`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("gaussian_blur.wgsl");

#[derive(Component)]
#[component(name = "Gaussian Blur")]
#[repr(C)]
pub struct GaussianBlur {
    #[field(min = 0.1, max = 20.0, speed = 0.1)]
    pub sigma: f32,
    /// Tap count per axis. Not inspectable — `FieldKind` has no `u32`, and the
    /// shader clamps it to 15 anyway — but it MUST be declared here, because the
    /// uniform block has it and the struct's layout is what the host uploads.
    /// Without it the shader read this slot from whatever followed the 4 bytes
    /// Rust supplied.
    #[field(skip)]
    pub kernel_size: u32,
}

impl Default for GaussianBlur {
    fn default() -> Self {
        Self {
            sigma: 2.0,
            // 9 taps per axis (81 samples). The shader clamps to 15, and the
            // loop is O(n²), so this is the quality/cost knee rather than the
            // ceiling.
            kernel_size: 9,
        }
    }
}

pub struct GaussianBlurPlugin;

impl Plugin for GaussianBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<GaussianBlur>("gaussian_blur", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GaussianBlurPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<GaussianBlur>(WGSL, "GaussianBlurSettings");
    }
}
