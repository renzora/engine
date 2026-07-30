//! Color Split post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_color_split`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("color_split.wgsl");

#[derive(Component)]
#[component(name = "Color Split")]
#[repr(C)]
pub struct ColorSplit {
    #[field(min = 0.0, max = 0.05, speed = 0.001)]
    pub offset_r: f32,
    #[field(min = 0.0, max = 0.05, speed = 0.001)]
    pub offset_b: f32,
    #[field(min = 0.0, max = 6.283, speed = 0.01)]
    pub angle: f32,
}

impl Default for ColorSplit {
    fn default() -> Self {
        Self {
            offset_r: 0.005,
            offset_b: 0.005,
            angle: 0.0,
        }
    }
}

pub struct ColorSplitPlugin;

impl Plugin for ColorSplitPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<ColorSplit>("color_split", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ColorSplitPlugin);
