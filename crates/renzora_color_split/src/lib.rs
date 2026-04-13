use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "color_split.wgsl", name = "Color Split", icon = "SQUARES_FOUR")]
pub struct ColorSplitSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.05, default = 0.005)]
    pub offset_r: f32,
    #[field(speed = 0.001, min = 0.0, max = 0.05, default = 0.005)]
    pub offset_b: f32,
    #[field(speed = 0.01, min = 0.0, max = 6.283, default = 0.0)]
    pub angle: f32,
}

#[derive(Default)]
pub struct ColorSplitPlugin;

impl Plugin for ColorSplitPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ColorSplitPlugin");
        bevy::asset::embedded_asset!(app, "color_split.wgsl");
        app.register_type::<ColorSplitSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ColorSplitSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ColorSplitSettings>();
    }
}

renzora::add!(ColorSplitPlugin);
