use bevy::prelude::*;
use serde;
use renzora::postprocess as renzora_postprocess;
#[cfg(feature = "editor")]
use renzora::editor as renzora_editor_framework;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "sketch.wgsl", name = "Sketch", icon = "PENCIL_LINE")]
pub struct SketchSettings {
    #[field(speed = 0.01, min = 0.0, max = 5.0, default = 1.5)]
    pub edge_strength: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.95)]
    pub paper_brightness: f32,
    #[field(speed = 0.1, min = 0.5, max = 5.0, default = 1.0)]
    pub line_density: f32,
}

#[derive(Default)]
pub struct SketchPlugin;

impl Plugin for SketchPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SketchPlugin");
        bevy::asset::embedded_asset!(app, "sketch.wgsl");
        app.register_type::<SketchSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SketchSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<SketchSettings>();
    }
}

renzora::add!(SketchPlugin);
