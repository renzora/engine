use bevy::prelude::*;
use serde;
use renzora_postprocess;
#[cfg(feature = "editor")]
use renzora_editor::AppEditorExt;

#[renzora_macros::post_process(shader = "emboss.wgsl", name = "Emboss", icon = "CUBE")]
pub struct EmbossSettings {
    #[field(speed = 0.01, min = 0.0, max = 3.0, default = 1.0)]
    pub strength: f32,
    #[field(name = "Mix", speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub mix_amount: f32,
}

pub struct EmbossPlugin;

impl Plugin for EmbossPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "emboss.wgsl");
        app.register_type::<EmbossSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<EmbossSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<EmbossSettings>();
    }
}
