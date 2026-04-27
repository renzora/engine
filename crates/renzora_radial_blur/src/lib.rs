use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "radial_blur.wgsl", name = "Radial Blur", icon = "CIRCLE_DASHED")]
pub struct RadialBlurSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.2, default = 0.02)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub center_x: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub center_y: f32,
    #[field(speed = 1.0, min = 4.0, max = 32.0, default = 8.0)]
    pub samples: f32,
}

#[derive(Default)]
pub struct RadialBlurPlugin;

impl Plugin for RadialBlurPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] RadialBlurPlugin");
        bevy::asset::embedded_asset!(app, "radial_blur.wgsl");
        app.register_type::<RadialBlurSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<RadialBlurSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<RadialBlurSettings>();
    }
}

renzora::add!(RadialBlurPlugin);
