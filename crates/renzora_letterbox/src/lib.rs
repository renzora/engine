use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "letterbox.wgsl", name = "Letterbox", icon = "ROWS")]
pub struct LetterboxSettings {
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.12)]
    pub bar_height: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.0)]
    pub softness: f32,
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 0.0, name = "Aspect Ratio")]
    pub aspect_ratio: f32,
}

#[derive(Default)]
pub struct LetterboxPlugin;

impl Plugin for LetterboxPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LetterboxPlugin");
        bevy::asset::embedded_asset!(app, "letterbox.wgsl");
        app.register_type::<LetterboxSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<LetterboxSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<LetterboxSettings>();
    }
}

renzora::add!(LetterboxPlugin);
