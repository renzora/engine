//! Tilt Shift post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_tilt_shift`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("tilt_shift.wgsl");

#[derive(Component)]
#[component(name = "Tilt Shift")]
#[repr(C)]
pub struct TiltShift {
    #[field(min = 0.0, max = 10.0, speed = 0.1)]
    pub blur_amount: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub focus_position: f32,
    #[field(min = 0.01, max = 0.5, speed = 0.01)]
    pub focus_width: f32,
    #[field(min = 0.01, max = 0.5, speed = 0.01)]
    pub focus_falloff: f32,
}

impl Default for TiltShift {
    fn default() -> Self {
        Self {
            blur_amount: 3.0,
            focus_position: 0.5,
            focus_width: 0.1,
            focus_falloff: 0.15,
        }
    }
}

pub struct TiltShiftPlugin;

impl Plugin for TiltShiftPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<TiltShift>("tilt_shift", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(TiltShiftPlugin);
