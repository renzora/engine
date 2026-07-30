//! Sobel Edge post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_sobel_edge`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("sobel_edge.wgsl");

#[derive(Component)]
#[component(name = "Sobel Edge")]
#[repr(C)]
pub struct SobelEdge {
    #[field(min = 0.0, max = 5.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub threshold: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub color_b: f32,
}

impl Default for SobelEdge {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            threshold: 0.1,
            color_r: 0.0,
            color_g: 1.0,
            color_b: 0.0,
        }
    }
}

pub struct SobelEdgePlugin;

impl Plugin for SobelEdgePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<SobelEdge>("sobel_edge", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SobelEdgePlugin);
