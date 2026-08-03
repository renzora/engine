//! God Rays post-process effect.
//!
//! Converted from `crates/renzora_god_rays`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("god_rays.wgsl");

#[derive(Component)]
#[component(name = "God Rays")]
#[repr(C)]
pub struct GodRays {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
    #[field(min = 0.9, max = 1.0, speed = 0.001)]
    pub decay: f32,
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub density: f32,
    #[field(min = -1.0, max = 2.0, speed = 0.01)]
    pub light_pos_x: f32,
    #[field(min = -1.0, max = 2.0, speed = 0.01)]
    pub light_pos_y: f32,
}

impl Default for GodRays {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            decay: 0.97,
            density: 1.0,
            light_pos_x: 0.5,
            light_pos_y: 0.3,
        }
    }
}

pub struct GodRaysPlugin;

impl Plugin for GodRaysPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<GodRays>("god_rays", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(GodRaysPlugin);
