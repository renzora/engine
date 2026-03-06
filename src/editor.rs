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
        use renzora_debugger::DebuggerPlugin;
        use renzora_gamepad::GamepadPlugin;

        app.add_plugins(SplashPlugin);
        app.add_plugins(RenzoraEditorPlugin);
        app.add_plugins(GridPlugin);
        app.add_plugins(CameraPlugin);
        app.add_plugins(KeybindingsPlugin);
        app.add_plugins(GizmoPlugin);
        app.add_plugins(ViewportPlugin);
        app.add_plugins(AssetBrowserPlugin);
        app.add_plugins(HierarchyPanelPlugin);
        app.add_plugins(InspectorPanelPlugin);
        app.add_plugins(TestComponentPlugin);
        app.add_plugins(ScenePlugin);
        app.add_plugins(ExportPlugin);
        app.add_plugins(MixerPlugin);
        app.add_plugins(ConsolePlugin);
        app.add_plugins(DebuggerPlugin);
        app.add_plugins(GamepadPlugin);
    }

    app.run();
}
