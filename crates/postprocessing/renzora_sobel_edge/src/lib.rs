use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "sobel_edge.wgsl", name = "Sobel Edge", icon = "SELECTION_ALL")]
pub struct SobelEdgeSettings {
    #[field(speed = 0.01, min = 0.0, max = 5.0, default = 1.0)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.1)]
    pub threshold: f32,
    #[field(skip, default = 0.0)]
    pub color_r: f32,
    #[field(skip, default = 1.0)]
    pub color_g: f32,
    #[field(skip, default = 0.0)]
    pub color_b: f32,
}

#[derive(Default)]
pub struct SobelEdgePlugin;

impl Plugin for SobelEdgePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SobelEdgePlugin");
        bevy::asset::embedded_asset!(app, "sobel_edge.wgsl");
        app.register_type::<SobelEdgeSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SobelEdgeSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<SobelEdgeSettings>();
    }
}

renzora::add!(SobelEdgePlugin);
