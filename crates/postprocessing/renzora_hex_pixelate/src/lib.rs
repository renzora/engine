use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "hex_pixelate.wgsl", name = "Hex Pixelate", icon = "HEXAGON")]
pub struct HexPixelateSettings {
    #[field(speed = 0.5, min = 2.0, max = 50.0, default = 10.0)]
    pub hex_size: f32,
}

#[derive(Default)]
pub struct HexPixelatePlugin;

impl Plugin for HexPixelatePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] HexPixelatePlugin");
        bevy::asset::embedded_asset!(app, "hex_pixelate.wgsl");
        app.register_type::<HexPixelateSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<HexPixelateSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<HexPixelateSettings>();
    }
}

renzora::add!(HexPixelatePlugin);
