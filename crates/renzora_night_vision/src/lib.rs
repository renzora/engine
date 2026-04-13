use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "night_vision.wgsl", name = "Night Vision", icon = "BINOCULARS")]
pub struct NightVisionSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 1.0)]
    pub intensity: f32,
    #[field(name = "Noise", speed = 0.01, min = 0.0, max = 1.0, default = 0.15)]
    pub noise_amount: f32,
    #[field(name = "Scanlines", speed = 0.01, min = 0.0, max = 1.0, default = 0.3)]
    pub scanline_amount: f32,
    #[field(speed = 0.05, min = 1.0, max = 10.0, default = 3.0)]
    pub color_amplification: f32,
    #[field(skip, default = 0.0)]
    pub time: f32,
}

#[derive(Default)]
pub struct NightVisionPlugin;

impl Plugin for NightVisionPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] NightVisionPlugin");
        bevy::asset::embedded_asset!(app, "night_vision.wgsl");
        app.register_type::<NightVisionSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<NightVisionSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        app.register_inspectable::<NightVisionSettings>();
    }
}

fn sync_time(time: Res<Time>, mut query: Query<&mut NightVisionSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

renzora::add!(NightVisionPlugin);
