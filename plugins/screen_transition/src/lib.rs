//! Screen Transition post-process effect.
//!
//! Converted from `crates/renzora_screen_transition`, which wrote its `PostProcessEffect`
//! impl and its `InspectorEntry` by hand rather than using `#[post_process]`.
//! The ranges below came from that entry's `FieldDef` list. See `plugins/crt` for
//! the conversion notes.

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("screen_transition.wgsl");

#[derive(Component)]
#[component(name = "Screen Transition")]
#[repr(C)]
pub struct ScreenTransition {
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub progress: f32,
    #[field(min = 0.0, max = 3.0, speed = 1.0)]
    pub mode: f32,
    #[field(min = 0.0, max = 3.0, speed = 1.0)]
    pub direction: f32,
    #[field(min = 0.0, max = 0.5, speed = 0.005)]
    pub smoothness: f32,
    #[field(skip)]
    pub color_r: f32,
    #[field(skip)]
    pub color_g: f32,
    #[field(skip)]
    pub color_b: f32,
}

impl Default for ScreenTransition {
    fn default() -> Self {
        Self {
            progress: 1.0,
            mode: 0.0,
            direction: 0.0,
            smoothness: 0.03,
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
        }
    }
}

pub struct ScreenTransitionPlugin;

impl Plugin for ScreenTransitionPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<ScreenTransition>("screen_transition", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(ScreenTransitionPlugin);
