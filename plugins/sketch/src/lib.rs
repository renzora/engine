//! Sketch post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_sketch`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("sketch.wgsl");

#[derive(Component)]
#[component(name = "Sketch")]
#[repr(C)]
pub struct Sketch {
    #[field(min = 0.0, max = 5.0, speed = 0.01)]
    pub edge_strength: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub paper_brightness: f32,
    #[field(min = 0.5, max = 5.0, speed = 0.1)]
    pub line_density: f32,
}

impl Default for Sketch {
    fn default() -> Self {
        Self {
            edge_strength: 1.5,
            paper_brightness: 0.95,
            line_density: 1.0,
        }
    }
}

pub struct SketchPlugin;

impl Plugin for SketchPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Sketch>("sketch", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SketchPlugin);
