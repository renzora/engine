use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "cross_hatch.wgsl", name = "Cross Hatch", icon = "HASH")]
pub struct CrossHatchSettings {
    #[field(speed = 0.5, min = 2.0, max = 100.0, default = 30.0)]
    pub density: f32,
    #[field(speed = 0.01, min = 0.01, max = 0.5, default = 0.1)]
    pub thickness: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.57, default = 0.785)]
    pub angle: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.9)]
    pub brightness: f32,
}

#[derive(Default)]
pub struct CrossHatchPlugin;

impl Plugin for CrossHatchPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CrossHatchPlugin");
        bevy::asset::embedded_asset!(app, "cross_hatch.wgsl");
        app.register_type::<CrossHatchSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<CrossHatchSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<CrossHatchSettings>();
    }
}

renzora::add!(CrossHatchPlugin);
