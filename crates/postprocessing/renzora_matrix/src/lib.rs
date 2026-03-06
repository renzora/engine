use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "matrix.wgsl", name = "Matrix Rain", icon = "CODE")]
pub struct MatrixSettings {
    #[field(speed = 0.05, min = 0.1, max = 10.0, default = 2.0)]
    pub speed: f32,
    #[field(speed = 0.5, min = 5.0, max = 50.0, default = 20.0)]
    pub density: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub glow: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.8)]
    pub trail_length: f32,
    #[field(skip, default = 0.0)]
    pub color_r: f32,
    #[field(skip, default = 1.0)]
    pub color_g: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

pub struct MatrixPlugin;

impl Plugin for MatrixPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "matrix.wgsl");
        app.register_type::<MatrixSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<MatrixSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<MatrixSettings>();
    }
}

fn sync_time(time: Res<Time>, mut query: Query<&mut MatrixSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}
