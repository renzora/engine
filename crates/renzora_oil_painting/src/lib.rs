use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "oil_painting.wgsl", name = "Oil Painting", icon = "PAINT_BUCKET")]
pub struct OilPaintingSettings {
    #[field(speed = 0.1, min = 1.0, max = 8.0, default = 3.0)]
    pub radius: f32,
    #[field(speed = 0.5, min = 4.0, max = 32.0, default = 8.0)]
    pub levels: f32,
}

#[derive(Default)]
pub struct OilPaintingPlugin;

impl Plugin for OilPaintingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] OilPaintingPlugin");
        bevy::asset::embedded_asset!(app, "oil_painting.wgsl");
        app.register_type::<OilPaintingSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<OilPaintingSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<OilPaintingSettings>();
    }
}

renzora::add!(OilPaintingPlugin);
