use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "sharpen.wgsl", name = "Sharpen", icon = "DIAMOND")]
pub struct SharpenSettings {
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 0.5)]
    pub strength: f32,
}

#[derive(Default)]
pub struct SharpenPlugin;

impl Plugin for SharpenPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SharpenPlugin");
        bevy::asset::embedded_asset!(app, "sharpen.wgsl");
        app.register_type::<SharpenSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SharpenSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<SharpenSettings>();
    }
}

renzora::add!(SharpenPlugin);
