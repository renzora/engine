use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "toon.wgsl", name = "Toon", icon = "PAINT_BRUSH")]
pub struct ToonSettings {
    #[field(speed = 0.1, min = 2.0, max = 16.0, default = 4.0)]
    pub levels: f32,
    #[field(speed = 0.005, min = 0.0, max = 1.0, default = 0.1)]
    pub edge_threshold: f32,
    #[field(speed = 0.05, min = 0.5, max = 5.0, default = 1.0)]
    pub edge_thickness: f32,
    #[field(speed = 0.02, min = 0.0, max = 3.0, default = 1.2)]
    pub saturation_boost: f32,
}

pub struct ToonPlugin;

impl Plugin for ToonPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ToonPlugin");
        bevy::asset::embedded_asset!(app, "toon.wgsl");
        app.register_type::<ToonSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ToonSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ToonSettings>();
    }
}
