use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "film_grain.wgsl", name = "Film Grain", icon = "FILM_STRIP")]
pub struct FilmGrainSettings {
    #[field(speed = 0.01, min = 0.0, max = 2.0, default = 0.3)]
    pub intensity: f32,
    #[field(speed = 0.1, min = 0.1, max = 10.0, default = 1.5)]
    pub grain_size: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

fn sync_time(time: Res<Time>, mut query: Query<&mut FilmGrainSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

pub struct FilmGrainPlugin;

impl Plugin for FilmGrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] FilmGrainPlugin");
        bevy::asset::embedded_asset!(app, "film_grain.wgsl");
        app.register_type::<FilmGrainSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<FilmGrainSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<FilmGrainSettings>();
    }
}
