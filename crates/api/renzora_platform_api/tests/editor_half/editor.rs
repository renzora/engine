//! The sample plugin's editor half, declared by `plugin!` rather than by
//! `main.rs`.

use bevy::prelude::*;

#[derive(Resource)]
pub struct EditorHalfRan {
    pub saw_runtime_half: bool,
}

pub(crate) fn register(app: &mut App) {
    let saw_runtime_half = app.world().contains_resource::<crate::RuntimeHalfRan>();
    app.insert_resource(EditorHalfRan { saw_runtime_half });
}
