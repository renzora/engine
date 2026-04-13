use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "grayscale.wgsl", name = "Grayscale", icon = "DROP_HALF_BOTTOM")]
pub struct GrayscaleSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
    #[field(skip, default = 0.2126)]
    pub luminance_r: f32,
    #[field(skip, default = 0.7152)]
    pub luminance_g: f32,
    #[field(skip, default = 0.0722)]
    pub luminance_b: f32,
}

#[derive(Default)]
pub struct GrayscalePlugin;

impl Plugin for GrayscalePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GrayscalePlugin");
        bevy::asset::embedded_asset!(app, "grayscale.wgsl");
        app.register_type::<GrayscaleSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<GrayscaleSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<GrayscaleSettings>();
    }
}

renzora::add!(GrayscalePlugin);
