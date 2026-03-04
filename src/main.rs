use bevy::prelude::*;
use renzora_runtime::RuntimePlugin;
use renzora_editor::RenzoraEditorPlugin;
use renzora_viewport::ViewportPlugin;
use renzora_asset_browser::AssetBrowserPlugin;
use renzora_hierarchy::HierarchyPanelPlugin;
use renzora_inspector::InspectorPanelPlugin;
use renzora_test_component::TestComponentPlugin;
use renzora_grid::GridPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RuntimePlugin)
        .add_plugins(GridPlugin)
        .add_plugins(ViewportPlugin)
        .add_plugins(AssetBrowserPlugin)
        .add_plugins(HierarchyPanelPlugin)
        .add_plugins(InspectorPanelPlugin)
        .add_plugins(TestComponentPlugin)
        .add_plugins(RenzoraEditorPlugin)
        .run();
}