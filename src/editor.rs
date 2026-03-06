fn main() {
    let mut app = renzora::build_runtime_app();

    #[cfg(feature = "editor")] {
        use renzora_editor::RenzoraEditorPlugin;
        use renzora_splash::SplashPlugin;
        use renzora_viewport::ViewportPlugin;
        use renzora_asset_browser::AssetBrowserPlugin;
        use renzora_hierarchy::HierarchyPanelPlugin;
        use renzora_inspector::InspectorPanelPlugin;
        use renzora_test_component::TestComponentPlugin;
        use renzora_grid::GridPlugin;
        use renzora_camera::CameraPlugin;
        use renzora_keybindings::KeybindingsPlugin;
        use renzora_gizmo::GizmoPlugin;
        use renzora_scene::ScenePlugin;
        use renzora_export::ExportPlugin;
        use renzora_mixer::MixerPlugin;
        use renzora_console::ConsolePlugin;

        app.add_plugins((
            SplashPlugin,
            RenzoraEditorPlugin,
            GridPlugin,
            CameraPlugin,
            KeybindingsPlugin,
            GizmoPlugin,
            ViewportPlugin,
            AssetBrowserPlugin,
            HierarchyPanelPlugin,
            InspectorPanelPlugin,
            TestComponentPlugin,
            ScenePlugin,
            ExportPlugin,
            MixerPlugin,
            ConsolePlugin,
        ));
    }

    app.run();
}
