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
