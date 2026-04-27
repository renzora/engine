use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "vibrance.wgsl", name = "Vibrance", icon = "PALETTE")]
pub struct VibranceSettings {
    #[field(speed = 0.01, min = -1.0, max = 2.0, default = 0.5)]
    pub intensity: f32,
}

#[derive(Default)]
pub struct VibrancePlugin;

impl Plugin for VibrancePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VibrancePlugin");
        bevy::asset::embedded_asset!(app, "vibrance.wgsl");
        app.register_type::<VibranceSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<VibranceSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<VibranceSettings>();
    }
}

renzora::add!(VibrancePlugin);
