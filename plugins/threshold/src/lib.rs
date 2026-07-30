//! Threshold post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_threshold`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("threshold.wgsl");

#[derive(Component)]
#[component(name = "Threshold")]
#[repr(C)]
pub struct Threshold {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub threshold: f32,
    #[field(min = 0.0, max = 0.5, speed = 0.01)]
    pub smoothness: f32,
}

impl Default for Threshold {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            smoothness: 0.05,
        }
    }
}

pub struct ThresholdPlugin;

impl Plugin for ThresholdPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Threshold>("threshold", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ThresholdPlugin);
