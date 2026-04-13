use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "dream.wgsl", name = "Dream", icon = "CLOUD")]
pub struct DreamSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.4)]
    pub intensity: f32,
    #[field(speed = 0.1, min = 1.0, max = 10.0, default = 3.0)]
    pub blur_radius: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub threshold: f32,
}

#[derive(Default)]
pub struct DreamPlugin;

impl Plugin for DreamPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DreamPlugin");
        bevy::asset::embedded_asset!(app, "dream.wgsl");
        app.register_type::<DreamSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<DreamSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<DreamSettings>();
    }
}

renzora::add!(DreamPlugin);
