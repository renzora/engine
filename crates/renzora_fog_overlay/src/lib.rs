use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "fog_overlay.wgsl", name = "Fog Overlay", icon = "CLOUD_FOG")]
pub struct FogOverlaySettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub density: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub height: f32,
    #[field(skip, default = 0.7)]
    pub color_r: f32,
    #[field(skip, default = 0.75)]
    pub color_g: f32,
    #[field(skip, default = 0.8)]
    pub color_b: f32,
}

#[derive(Default)]
pub struct FogOverlayPlugin;

impl Plugin for FogOverlayPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] FogOverlayPlugin");
        bevy::asset::embedded_asset!(app, "fog_overlay.wgsl");
        app.register_type::<FogOverlaySettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<FogOverlaySettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<FogOverlaySettings>();
    }
}

renzora::add!(FogOverlayPlugin);
