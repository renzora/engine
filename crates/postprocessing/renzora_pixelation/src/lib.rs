use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "pixelation.wgsl", name = "Pixelation", icon = "GRID_FOUR")]
pub struct PixelationSettings {
    #[field(speed = 0.5, min = 1.0, max = 64.0, default = 4.0)]
    pub pixel_size: f32,
}

pub struct PixelationPlugin;

impl Plugin for PixelationPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PixelationPlugin");
        bevy::asset::embedded_asset!(app, "pixelation.wgsl");
        app.register_type::<PixelationSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<PixelationSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<PixelationSettings>();
    }
}
