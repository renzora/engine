use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "pillowbox.wgsl", name = "Pillarbox", icon = "COLUMNS")]
pub struct PillowboxSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.15)]
    pub bar_width: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.0)]
    pub softness: f32,
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 0.0, name = "Aspect Ratio")]
    pub aspect_ratio: f32,
}

#[derive(Default)]
pub struct PillowboxPlugin;

impl Plugin for PillowboxPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PillowboxPlugin");
        bevy::asset::embedded_asset!(app, "pillowbox.wgsl");
        app.register_type::<PillowboxSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<PillowboxSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<PillowboxSettings>();
    }
}

renzora::add!(PillowboxPlugin);
