mod primitives;
mod setup;

pub use primitives::{spawn_primitive, PrimitiveType};
#[allow(unused_imports)]
pub use setup::{setup_editor_camera, EditorOnly, UiCamera};

use bevy::prelude::*;

#[allow(dead_code)]
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, _app: &mut App) {
        // Scene loading is now done via OnEnter(AppState::Editor) in main.rs
    }
}
