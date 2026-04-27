use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "kaleidoscope.wgsl", name = "Kaleidoscope", icon = "FLOWER_LOTUS")]
pub struct KaleidoscopeSettings {
    #[field(speed = 0.1, min = 2.0, max = 32.0, default = 6.0)]
    pub segments: f32,
    #[field(speed = 0.01, min = 0.0, max = 6.283, default = 0.0)]
    pub rotation: f32,
    #[field(skip, default = 0.5)]
    pub center_x: f32,
    #[field(skip, default = 0.5)]
    pub center_y: f32,
}

#[derive(Default)]
pub struct KaleidoscopePlugin;

impl Plugin for KaleidoscopePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] KaleidoscopePlugin");
        bevy::asset::embedded_asset!(app, "kaleidoscope.wgsl");
        app.register_type::<KaleidoscopeSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<KaleidoscopeSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<KaleidoscopeSettings>();
    }
}

renzora::add!(KaleidoscopePlugin);
