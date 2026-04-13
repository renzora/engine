use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "swirl.wgsl", name = "Swirl", icon = "SPIRAL")]
pub struct SwirlSettings {
    #[field(speed = 0.01, min = -10.0, max = 10.0, default = 3.0)]
    pub angle: f32,
    #[field(speed = 0.01, min = 0.01, max = 2.0, default = 0.5)]
    pub radius: f32,
    #[field(skip, default = 0.5)]
    pub center_x: f32,
    #[field(skip, default = 0.5)]
    pub center_y: f32,
}

#[derive(Default)]
pub struct SwirlPlugin;

impl Plugin for SwirlPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SwirlPlugin");
        bevy::asset::embedded_asset!(app, "swirl.wgsl");
        app.register_type::<SwirlSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SwirlSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<SwirlSettings>();
    }
}

renzora::add!(SwirlPlugin);
