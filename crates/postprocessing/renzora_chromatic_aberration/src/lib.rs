use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "chromatic_aberration.wgsl", name = "Chromatic Aberration", icon = "RAINBOW")]
pub struct ChromaticAberrationSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.1, default = 0.005)]
    pub intensity: f32,
    #[field(speed = 1.0, min = 1.0, max = 16.0, default = 3.0)]
    pub samples: f32,
    #[field(skip, default = 1.0)]
    pub direction_x: f32,
    #[field(skip, default = 0.0)]
    pub direction_y: f32,
}

pub struct ChromaticAberrationPlugin;

impl Plugin for ChromaticAberrationPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ChromaticAberrationPlugin");
        bevy::asset::embedded_asset!(app, "chromatic_aberration.wgsl");
        app.register_type::<ChromaticAberrationSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ChromaticAberrationSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ChromaticAberrationSettings>();
    }
}
