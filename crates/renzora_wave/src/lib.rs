use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "wave.wgsl", name = "Wave", icon = "WAVE_SINE")]
pub struct WaveSettings {
    #[field(speed = 0.001, min = 0.0, max = 0.1, default = 0.01)]
    pub amplitude: f32,
    #[field(speed = 0.5, min = 1.0, max = 50.0, default = 10.0)]
    pub frequency: f32,
    #[field(speed = 0.1, min = 0.1, max = 10.0, default = 2.0)]
    pub speed: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut WaveSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[derive(Default)]
pub struct WavePlugin;

impl Plugin for WavePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] WavePlugin");
        bevy::asset::embedded_asset!(app, "wave.wgsl");
        app.register_type::<WaveSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<WaveSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<WaveSettings>();
    }
}

renzora::add!(WavePlugin);
