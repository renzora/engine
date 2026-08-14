#![no_std]
//! Palette Quantization post-process effect.
//!
//! Converted from `crates/renzora_palette_quantization`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("palette_quantization.wgsl");

#[derive(Component)]
#[component(name = "Palette Quantization")]
#[repr(C)]
pub struct PaletteQuantization {
    /// Quantization levels per channel. Not inspectable — `FieldKind` has no
    /// `u32` — but it MUST come FIRST, because that is where the uniform block
    /// has it. While it was missing, the shader read `num_colors` from
    /// `dithering`'s bit pattern (0.5 reinterprets as 1,056,964,608 levels,
    /// which quantizes to nothing) and read `dithering` from whatever followed.
    /// The effect did nothing at all.
    #[field(skip)]
    pub num_colors: u32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub dithering: f32,
}

impl Default for PaletteQuantization {
    fn default() -> Self {
        Self {
            // 8 levels per channel — 512 colours, the classic retro-palette look
            // this effect is for. The shader floors it at 2.
            num_colors: 8,
            dithering: 0.5,
        }
    }
}

pub struct PaletteQuantizationPlugin;

impl Plugin for PaletteQuantizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<PaletteQuantization>("palette_quantization", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(PaletteQuantizationPlugin);

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
        renzora_plugin::uniform_check::assert_uniform_matches::<PaletteQuantization>(WGSL, "PaletteQuantizationSettings");
    }
}
