//! Kuwahara post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_kuwahara`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("kuwahara.wgsl");

#[derive(Component)]
#[component(name = "Kuwahara")]
#[repr(C)]
pub struct Kuwahara {
    #[field(min = 1.0, max = 8.0, speed = 0.1)]
    pub radius: f32,
}

impl Default for Kuwahara {
    fn default() -> Self {
        Self {
            radius: 3.0,
        }
    }
}

pub struct KuwaharaPlugin;

impl Plugin for KuwaharaPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Kuwahara>("kuwahara", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(KuwaharaPlugin);
