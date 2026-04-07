use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "distortion.wgsl", name = "Distortion", icon = "WAVE_SINE")]
pub struct DistortionSettings {
    #[field(speed = 0.01, min = 0.0, max = 2.0, default = 0.02)]
    pub intensity: f32,
    #[field(speed = 0.01, min = 0.0, max = 10.0, default = 1.0)]
    pub speed: f32,
    #[field(speed = 0.1, min = 0.1, max = 50.0, default = 10.0)]
    pub scale: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut DistortionSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[derive(Default)]
pub struct DistortionPlugin;

impl Plugin for DistortionPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DistortionPlugin");
        bevy::asset::embedded_asset!(app, "distortion.wgsl");
        app.register_type::<DistortionSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<DistortionSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<DistortionSettings>();
    }
}

renzora::add!(DistortionPlugin);
