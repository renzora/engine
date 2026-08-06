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
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub dithering: f32,
}

impl Default for PaletteQuantization {
    fn default() -> Self {
        Self {
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
