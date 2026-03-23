use bevy::prelude::*;
use renzora_editor::RenzoraEditorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RenzoraEditorPlugin)
        .run();
}
