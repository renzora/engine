use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "heat_haze.wgsl", name = "Heat Haze", icon = "FIRE")]
pub struct HeatHazeSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.15)]
    pub intensity: f32,
    #[field(speed = 0.1, min = 0.1, max = 10.0, default = 2.0)]
    pub speed: f32,
    #[field(speed = 0.1, min = 1.0, max = 100.0, default = 20.0)]
    pub scale: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut HeatHazeSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[derive(Default)]
pub struct HeatHazePlugin;

impl Plugin for HeatHazePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] HeatHazePlugin");
        bevy::asset::embedded_asset!(app, "heat_haze.wgsl");
        app.register_type::<HeatHazeSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<HeatHazeSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<HeatHazeSettings>();
    }
}

renzora::add!(HeatHazePlugin);
