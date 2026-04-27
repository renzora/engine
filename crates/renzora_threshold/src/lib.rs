use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "threshold.wgsl", name = "Threshold", icon = "CIRCLE_HALF")]
pub struct ThresholdSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub threshold: f32,
    #[field(speed = 0.01, min = 0.0, max = 0.5, default = 0.05)]
    pub smoothness: f32,
}

#[derive(Default)]
pub struct ThresholdPlugin;

impl Plugin for ThresholdPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ThresholdPlugin");
        bevy::asset::embedded_asset!(app, "threshold.wgsl");
        app.register_type::<ThresholdSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ThresholdSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ThresholdSettings>();
    }
}

renzora::add!(ThresholdPlugin);
