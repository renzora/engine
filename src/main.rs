use bevy::prelude::*;
use renzora_editor::RenzoraEditorPlugin;
use renzora_hierarchy::HierarchyPanelPlugin;
use renzora_widget_gallery::WidgetGalleryPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RenzoraEditorPlugin)
        .add_plugins(HierarchyPanelPlugin)
        .add_plugins(WidgetGalleryPlugin)
        .run();
}
