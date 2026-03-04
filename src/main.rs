use bevy::prelude::*;
use renzora_runtime::RuntimePlugin;
use renzora_editor::RenzoraEditorPlugin;
use renzora_viewport::ViewportPlugin;
use renzora_asset_browser::AssetBrowserPlugin;
use renzora_hierarchy::HierarchyPanelPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RuntimePlugin)
        .add_plugins(RenzoraEditorPlugin)
        .add_plugins(ViewportPlugin)
        .add_plugins(AssetBrowserPlugin)
        .add_plugins(HierarchyPanelPlugin)
        .run();
}