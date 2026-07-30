//! Gaussian Blur post-process effect.
//!
//! Converted from `crates/renzora_gaussian_blur`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("gaussian_blur.wgsl");

#[derive(Component)]
#[component(name = "Gaussian Blur")]
#[repr(C)]
pub struct GaussianBlur {
    #[field(min = 0.1, max = 20.0, speed = 0.1)]
    pub sigma: f32,
}

impl Default for GaussianBlur {
    fn default() -> Self {
        Self {
            sigma: 2.0,
        }
    }
}

pub struct GaussianBlurPlugin;

impl Plugin for GaussianBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<GaussianBlur>("gaussian_blur", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GaussianBlurPlugin);
