use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "rain.wgsl", name = "Rain", icon = "DROP")]
pub struct RainSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub intensity: f32,
    #[field(speed = 0.1, min = 0.1, max = 5.0, default = 1.0)]
    pub speed: f32,
    #[field(speed = 0.1, min = 1.0, max = 20.0, default = 8.0)]
    pub drop_size: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut RainSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[derive(Default)]
pub struct RainPlugin;

impl Plugin for RainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] RainPlugin");
        bevy::asset::embedded_asset!(app, "rain.wgsl");
        app.register_type::<RainSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<RainSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<RainSettings>();
    }
}

renzora::add!(RainPlugin);
