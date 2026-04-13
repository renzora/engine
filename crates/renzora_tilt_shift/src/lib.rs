use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "tilt_shift.wgsl", name = "Tilt Shift", icon = "BINOCULARS")]
pub struct TiltShiftSettings {
    #[field(speed = 0.1, min = 0.0, max = 10.0, default = 3.0)]
    pub blur_amount: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub focus_position: f32,
    #[field(speed = 0.01, min = 0.01, max = 0.5, default = 0.1)]
    pub focus_width: f32,
    #[field(speed = 0.01, min = 0.01, max = 0.5, default = 0.15)]
    pub focus_falloff: f32,
}

#[derive(Default)]
pub struct TiltShiftPlugin;

impl Plugin for TiltShiftPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TiltShiftPlugin");
        bevy::asset::embedded_asset!(app, "tilt_shift.wgsl");
        app.register_type::<TiltShiftSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<TiltShiftSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<TiltShiftSettings>();
    }
}

renzora::add!(TiltShiftPlugin);
