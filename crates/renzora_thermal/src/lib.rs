use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "thermal.wgsl", name = "Thermal Vision", icon = "THERMOMETER_HOT")]
pub struct ThermalSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.1, max = 3.0, default = 1.5)]
    pub contrast: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub cold_threshold: f32,
}

#[derive(Default)]
pub struct ThermalPlugin;

impl Plugin for ThermalPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ThermalPlugin");
        bevy::asset::embedded_asset!(app, "thermal.wgsl");
        app.register_type::<ThermalSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ThermalSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ThermalSettings>();
    }
}

renzora::add!(ThermalPlugin);
