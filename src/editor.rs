fn main() {
    let mut app = renzora::build_runtime_app();

    #[cfg(feature = "editor")] {
        app.add_plugins(renzora_splash::SplashPlugin);
        app.add_plugins(renzora_editor::RenzoraEditorPlugin);
        app.add_plugins(renzora_grid::GridPlugin);
        app.add_plugins(renzora_camera::CameraPlugin);
        app.add_plugins(renzora_keybindings::KeybindingsPlugin);
        app.add_plugins(renzora_viewport::ViewportPlugin);
        app.add_plugins(renzora_asset_browser::AssetBrowserPlugin);
        app.add_plugins(renzora_hierarchy::HierarchyPanelPlugin);
        app.add_plugins(renzora_inspector::InspectorPanelPlugin);
        app.add_plugins(renzora_test_component::TestComponentPlugin);
        app.add_plugins(renzora_scene::ScenePlugin);
        app.add_plugins(renzora_export::ExportPlugin);
        app.add_plugins(renzora_mixer::MixerPlugin);
        app.add_plugins(renzora_console::ConsolePlugin);
        app.add_plugins(renzora_debugger::DebuggerPlugin);
        app.add_plugins(renzora_physics_playground::PhysicsPanelPlugin);
        app.add_plugins(renzora_gamepad::GamepadPlugin);
        app.add_plugins(renzora_gizmo::GizmoPlugin);
        app.add_plugins(renzora_settings::SettingsPlugin);
    }

    app.run();
}
