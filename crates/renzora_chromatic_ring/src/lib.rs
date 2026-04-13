use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "chromatic_ring.wgsl", name = "Chromatic Ring", icon = "CIRCLE")]
pub struct ChromaticRingSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.05, default = 0.008)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 2.0, default = 0.8)]
    pub radius: f32,
    #[field(speed = 0.01, min = 0.01, max = 1.0, default = 0.4)]
    pub falloff: f32,
}

#[derive(Default)]
pub struct ChromaticRingPlugin;

impl Plugin for ChromaticRingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ChromaticRingPlugin");
        bevy::asset::embedded_asset!(app, "chromatic_ring.wgsl");
        app.register_type::<ChromaticRingSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ChromaticRingSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<ChromaticRingSettings>();
    }
}

renzora::add!(ChromaticRingPlugin);
