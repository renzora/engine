//! Underwater post-process effect.
//!
//! Converted from `crates/renzora_underwater`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("underwater.wgsl");

#[derive(Component)]
#[component(name = "Underwater")]
#[repr(C)]
pub struct Underwater {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub distortion: f32,
    #[field(skip)]
    pub tint_r: f32,
    #[field(skip)]
    pub tint_g: f32,
    #[field(skip)]
    pub tint_b: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub tint_strength: f32,
    #[field(min = 0.0, max = 10.0, speed = 0.01)]
    pub wave_speed: f32,
    #[field(min = 0.1, max = 50.0, speed = 0.1)]
    pub wave_scale: f32,
    #[field(skip)]
    pub time: f32,
}

impl Default for Underwater {
    fn default() -> Self {
        Self {
            distortion: 0.02,
            tint_r: 0.0,
            tint_g: 0.3,
            tint_b: 0.5,
            tint_strength: 0.3,
            wave_speed: 1.0,
            wave_scale: 10.0,
            time: 0.0,
        }
    }
}

pub struct UnderwaterPlugin;

impl Plugin for UnderwaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Underwater>("underwater", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(UnderwaterPlugin);
