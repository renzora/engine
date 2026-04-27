use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "invert.wgsl", name = "Invert", icon = "SWAP")]
pub struct InvertSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
}

#[derive(Default)]
pub struct InvertPlugin;

impl Plugin for InvertPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] InvertPlugin");
        bevy::asset::embedded_asset!(app, "invert.wgsl");
        app.register_type::<InvertSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<InvertSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<InvertSettings>();
    }
}

renzora::add!(InvertPlugin);
