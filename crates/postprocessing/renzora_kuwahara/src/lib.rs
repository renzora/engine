use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "kuwahara.wgsl", name = "Kuwahara", icon = "PAINT_BRUSH")]
pub struct KuwaharaSettings {
    #[field(speed = 0.1, min = 1.0, max = 8.0, default = 3.0)]
    pub radius: f32,
}

#[derive(Default)]
pub struct KuwaharaPlugin;

impl Plugin for KuwaharaPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] KuwaharaPlugin");
        bevy::asset::embedded_asset!(app, "kuwahara.wgsl");
        app.register_type::<KuwaharaSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<KuwaharaSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<KuwaharaSettings>();
    }
}

renzora::add!(KuwaharaPlugin);
