//! Sharpen post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_sharpen`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("sharpen.wgsl");

#[derive(Component)]
#[component(name = "Sharpen")]
#[repr(C)]
pub struct Sharpen {
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub strength: f32,
}

impl Default for Sharpen {
    fn default() -> Self {
        Self {
            strength: 0.5,
        }
    }
}

pub struct SharpenPlugin;

impl Plugin for SharpenPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Sharpen>("sharpen", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SharpenPlugin);
