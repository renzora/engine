use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "halftone.wgsl", name = "Halftone", icon = "DOTS_NINE")]
pub struct HalftoneSettings {
    #[field(speed = 0.1, min = 2.0, max = 20.0, default = 4.0)]
    pub dot_size: f32,
    #[field(speed = 0.01, min = 0.0, max = 3.14159, default = 0.785)]
    pub angle: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
}

#[derive(Default)]
pub struct HalftonePlugin;

impl Plugin for HalftonePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] HalftonePlugin");
        bevy::asset::embedded_asset!(app, "halftone.wgsl");
        app.register_type::<HalftoneSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<HalftoneSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<HalftoneSettings>();
    }
}

renzora::add!(HalftonePlugin);
