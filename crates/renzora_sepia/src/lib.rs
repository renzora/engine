use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "sepia.wgsl", name = "Sepia", icon = "IMAGE")]
pub struct SepiaSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
    #[field(skip, default = 1.2)]
    pub tone_r: f32,
    #[field(skip, default = 1.0)]
    pub tone_g: f32,
    #[field(skip, default = 0.8)]
    pub tone_b: f32,
}

#[derive(Default)]
pub struct SepiaPlugin;

impl Plugin for SepiaPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SepiaPlugin");
        bevy::asset::embedded_asset!(app, "sepia.wgsl");
        app.register_type::<SepiaSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SepiaSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<SepiaSettings>();
    }
}

renzora::add!(SepiaPlugin);
