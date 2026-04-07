use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "posterize.wgsl", name = "Posterize", icon = "STAIRS")]
pub struct PosterizeSettings {
    #[field(speed = 1.0, min = 2.0, max = 64.0, default = 8.0)]
    pub levels: f32,
}

#[derive(Default)]
pub struct PosterizePlugin;

impl Plugin for PosterizePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PosterizePlugin");
        bevy::asset::embedded_asset!(app, "posterize.wgsl");
        app.register_type::<PosterizeSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<PosterizeSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<PosterizeSettings>();
    }
}

renzora::add!(PosterizePlugin);
