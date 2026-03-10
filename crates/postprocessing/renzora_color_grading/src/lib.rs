use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "color_grading.wgsl", name = "Color Grading", icon = "PALETTE")]
pub struct ColorGradingSettings {
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 1.0)]
    pub brightness: f32,
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 1.0)]
    pub contrast: f32,
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 1.0)]
    pub saturation: f32,
    #[field(speed = 0.01, min = 0.1, max = 3.0, default = 1.0)]
    pub gamma: f32,
    #[field(speed = 0.01, min = -1.0, max = 1.0, default = 0.0)]
    pub temperature: f32,
    #[field(speed = 0.01, min = -1.0, max = 1.0, default = 0.0)]
    pub tint: f32,
}

pub struct ColorGradingPlugin;

impl Plugin for ColorGradingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ColorGradingPlugin");
        bevy::asset::embedded_asset!(app, "color_grading.wgsl");
        app.register_type::<ColorGradingSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ColorGradingSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ColorGradingSettings>();
    }
}
