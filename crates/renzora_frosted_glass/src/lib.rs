use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "frosted_glass.wgsl", name = "Frosted Glass", icon = "SNOWFLAKE")]
pub struct FrostedGlassSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.05, default = 0.01)]
    pub intensity: f32,
    #[field(speed = 0.5, min = 1.0, max = 50.0, default = 10.0)]
    pub scale: f32,
}

#[derive(Default)]
pub struct FrostedGlassPlugin;

impl Plugin for FrostedGlassPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] FrostedGlassPlugin");
        bevy::asset::embedded_asset!(app, "frosted_glass.wgsl");
        app.register_type::<FrostedGlassSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<FrostedGlassSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<FrostedGlassSettings>();
    }
}

renzora::add!(FrostedGlassPlugin);
