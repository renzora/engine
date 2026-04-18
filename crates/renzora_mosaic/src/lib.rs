use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "mosaic.wgsl", name = "Mosaic", icon = "GRID_FOUR")]
pub struct MosaicSettings {
    #[field(speed = 0.5, min = 4.0, max = 200.0, default = 40.0)]
    pub tile_size: f32,
    #[field(speed = 0.01, min = 0.0, max = 0.5, default = 0.05)]
    pub edge_thickness: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub roundness: f32,
}

#[derive(Default)]
pub struct MosaicPlugin;

impl Plugin for MosaicPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MosaicPlugin");
        bevy::asset::embedded_asset!(app, "mosaic.wgsl");
        app.register_type::<MosaicSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<MosaicSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<MosaicSettings>();
    }
}

renzora::add!(MosaicPlugin);
