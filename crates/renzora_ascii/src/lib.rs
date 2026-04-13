use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "ascii.wgsl", name = "ASCII", icon = "TEXT_AA")]
pub struct AsciiSettings {
    #[field(speed = 0.5, min = 2.0, max = 32.0, default = 8.0)]
    pub char_size: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub color_mix: f32,
    #[field(speed = 0.01, min = 0.5, max = 3.0, default = 1.2)]
    pub contrast: f32,
}

#[derive(Default)]
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AsciiPlugin");
        bevy::asset::embedded_asset!(app, "ascii.wgsl");
        app.register_type::<AsciiSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<AsciiSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<AsciiSettings>();
    }
}

renzora::add!(AsciiPlugin);
