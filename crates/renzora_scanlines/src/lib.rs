use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "scanlines.wgsl", name = "Scanlines", icon = "BARCODE")]
pub struct ScanlinesSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.15)]
    pub intensity: f32,
    #[field(speed = 10.0, min = 10.0, max = 2000.0, default = 800.0)]
    pub count: f32,
    #[field(speed = 0.1, min = 0.0, max = 10.0, default = 0.0)]
    pub speed: f32,
}

#[derive(Default)]
pub struct ScanlinesPlugin;

impl Plugin for ScanlinesPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ScanlinesPlugin");
        bevy::asset::embedded_asset!(app, "scanlines.wgsl");
        app.register_type::<ScanlinesSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ScanlinesSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ScanlinesSettings>();
    }
}

renzora::add!(ScanlinesPlugin);
