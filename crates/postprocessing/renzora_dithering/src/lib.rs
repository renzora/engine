use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "dithering.wgsl", name = "Dithering", icon = "GRID_FOUR")]
pub struct DitheringSettings {
    #[field(speed = 0.5, min = 2.0, max = 32.0, default = 8.0)]
    pub color_depth: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
}

pub struct DitheringPlugin;

impl Plugin for DitheringPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "dithering.wgsl");
        app.register_type::<DitheringSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<DitheringSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<DitheringSettings>();
    }
}
