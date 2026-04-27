use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "light_streaks.wgsl", name = "Light Streaks", icon = "SUN")]
pub struct LightStreaksSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.4)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.7)]
    pub threshold: f32,
    #[field(speed = 1.0, min = 4.0, max = 32.0, default = 12.0)]
    pub samples: f32,
    #[field(speed = 0.01, min = 0.0, max = 6.283, default = 0.0)]
    pub direction: f32,
}

#[derive(Default)]
pub struct LightStreaksPlugin;

impl Plugin for LightStreaksPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LightStreaksPlugin");
        bevy::asset::embedded_asset!(app, "light_streaks.wgsl");
        app.register_type::<LightStreaksSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<LightStreaksSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<LightStreaksSettings>();
    }
}

renzora::add!(LightStreaksPlugin);
