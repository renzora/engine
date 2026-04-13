use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "crt.wgsl", name = "CRT", icon = "MONITOR")]
pub struct CrtSettings {
    #[field(speed = 0.01, min = 0.0, max = 2.0, default = 0.3)]
    pub scanline_intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.02)]
    pub curvature: f32,
    #[field(speed = 0.001, min = 0.0, max = 0.1, default = 0.003)]
    pub chromatic_amount: f32,
    #[field(speed = 0.01, min = 0.0, max = 2.0, default = 0.5)]
    pub vignette_amount: f32,
}

#[derive(Default)]
pub struct CrtPlugin;

impl Plugin for CrtPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CrtPlugin");
        bevy::asset::embedded_asset!(app, "crt.wgsl");
        app.register_type::<CrtSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<CrtSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<CrtSettings>();
    }
}

renzora::add!(CrtPlugin);
