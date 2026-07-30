//! Mosaic post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_mosaic`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("mosaic.wgsl");

#[derive(Component)]
#[component(name = "Mosaic")]
#[repr(C)]
pub struct Mosaic {
    #[field(min = 4.0, max = 200.0, speed = 0.5)]
    pub tile_size: f32,
    #[field(min = 0.0, max = 0.5, speed = 0.01)]
    pub edge_thickness: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub roundness: f32,
}

impl Default for Mosaic {
    fn default() -> Self {
        Self {
            tile_size: 40.0,
            edge_thickness: 0.05,
            roundness: 0.3,
        }
    }
}

pub struct MosaicPlugin;

impl Plugin for MosaicPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Mosaic>("mosaic", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(MosaicPlugin);
