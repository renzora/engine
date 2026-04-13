use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "glitch.wgsl", name = "Glitch", icon = "LIGHTNING")]
pub struct GlitchSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub intensity: f32,
    #[field(speed = 1.0, min = 4.0, max = 64.0, default = 16.0)]
    pub block_size: f32,
    #[field(speed = 0.001, min = 0.0, max = 0.1, default = 0.01)]
    pub color_drift: f32,
    #[field(speed = 0.1, min = 0.1, max = 20.0, default = 5.0)]
    pub speed: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut GlitchSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[derive(Default)]
pub struct GlitchPlugin;

impl Plugin for GlitchPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GlitchPlugin");
        bevy::asset::embedded_asset!(app, "glitch.wgsl");
        app.register_type::<GlitchSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<GlitchSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<GlitchSettings>();
    }
}

renzora::add!(GlitchPlugin);
