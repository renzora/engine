use bevy::prelude::*;
use renzora_app::{renzora_shared, init_app, add_default_rendering, add_engine_plugins};

// ── WASM runtime (no editor) ──────────────────────────────────────────────

#[cfg(all(target_arch = "wasm32", not(feature = "editor")))]
fn main() {}

#[cfg(all(target_arch = "wasm32", not(feature = "editor")))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_rpak(data: &[u8]) {
    renzora_shared::renzora_engine::vfs::set_wasm_rpak(data.to_vec());
}

#[cfg(all(target_arch = "wasm32", not(feature = "editor")))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start() {
    let mut app = renzora_app::build_runtime_app();
    app.run();
}

// ── Native entry point ────────────────────────────────────────────────────

#[cfg(not(all(target_arch = "wasm32", not(feature = "editor"))))]
fn main() {
    renzora_shared::renzora_engine::crash::install_panic_hook();

    // Detect mode from binary name
    let is_editor = is_editor_binary();

    // Check for --project arg (passed by restart after splash)
    let project_path = std::env::args()
        .skip_while(|a| a != "--project")
        .nth(1)
        .map(std::path::PathBuf::from);

    if let Some(ref p) = project_path {
        log::info!("[ENGINE] --project arg: {:?}", p);
    }

    // No --project arg = show splash, pick project, restart with --project
    #[cfg(feature = "editor")]
    if is_editor && project_path.is_none() {
        run_splash();
        return;
    }

    // Determine plugins directory
    let plugins_dir = project_path.as_ref().map(|p| p.join("plugins"));

    // Phase 1: Create app base (asset reader)
    let mut app = init_app();

    // Phase 2: Add default rendering
    add_default_rendering(&mut app);

    // Phase 3: Add engine plugins
    add_engine_plugins(&mut app);

    app.add_plugins(renzora_shared::renzora_engine::crash::CrashReportPlugin);

    #[cfg(feature = "editor")]
    if is_editor {
        // Editor framework + infrastructure
        app.add_plugins(renzora_shared::renzora_splash::SplashPlugin);
        app.add_plugins(renzora_shared::renzora_editor_framework::RenzoraEditorPlugin);
        app.add_plugins(renzora_shared::renzora_grid::GridPlugin);
        app.add_plugins(renzora_shared::renzora_keybindings::KeybindingsPlugin);
        app.add_plugins(renzora_shared::renzora_viewport::ViewportPlugin);
        app.add_plugins(renzora_shared::renzora_camera::CameraPlugin);
        app.add_plugins(renzora_shared::renzora_gizmo::GizmoPlugin);
        app.add_plugins(renzora_shared::renzora_scene::ScenePlugin);

        // Editor panels (baked in)
        app.add_plugins(renzora_shared::renzora_console::ConsolePlugin);
        app.add_plugins(renzora_shared::renzora_inspector::InspectorPanelPlugin);
        app.add_plugins(renzora_shared::renzora_hierarchy::HierarchyPanelPlugin);
        app.add_plugins(renzora_shared::renzora_asset_browser::AssetBrowserPlugin);
        app.add_plugins(renzora_shared::renzora_export::ExportPlugin);
        app.add_plugins(renzora_shared::renzora_import_ui::ImportPlugin);
        app.add_plugins(renzora_shared::renzora_settings::SettingsPlugin);
        app.add_plugins(renzora_shared::renzora_material_editor::MaterialEditorPlugin);
        app.add_plugins(renzora_shared::renzora_shader_editor::ShaderEditorPlugin);
        app.add_plugins(renzora_shared::renzora_blueprint_editor::BlueprintEditorPlugin);
        app.add_plugins(renzora_shared::renzora_particle_editor::ParticleEditorPlugin);
        app.add_plugins(renzora_shared::renzora_terrain_editor::TerrainEditorPlugin);
        app.add_plugins(renzora_shared::renzora_foliage_editor::FoliageEditorPlugin);
        app.add_plugins(renzora_shared::renzora_animation_editor::AnimationEditorPlugin);
        app.add_plugins(renzora_shared::renzora_network_editor::NetworkEditorPlugin);
        app.add_plugins(renzora_shared::renzora_lifecycle_editor::LifecycleEditorPlugin);
        app.add_plugins(renzora_shared::renzora_code_editor::CodeEditorPlugin);
        app.add_plugins(renzora_shared::renzora_script_variables::ScriptVariablesPlugin);
        app.add_plugins(renzora_shared::renzora_mixer::MixerPlugin);
        app.add_plugins(renzora_shared::renzora_daw::DawPlugin);
        app.add_plugins(renzora_shared::renzora_hub::HubPlugin);
        app.add_plugins(renzora_shared::renzora_auth::AuthPlugin);
        app.add_plugins(renzora_shared::renzora_level_presets::LevelPresetsPlugin);
        app.add_plugins(renzora_shared::renzora_physics_playground::PhysicsPanelPlugin);
        app.add_plugins(renzora_shared::renzora_gamepad::GamepadPlugin);
        app.add_plugins(renzora_shared::renzora_debugger::DebuggerPlugin);
        app.add_plugins(renzora_shared::renzora_widget_gallery::WidgetGalleryPlugin);
        app.add_plugins(renzora_shared::renzora_tutorial::TutorialPlugin);
        app.add_plugins(renzora_shared::renzora_test_component::TestComponentPlugin);
        app.add_plugins(renzora_shared::renzora_test_extension::TestExtensionPlugin);

        app.insert_resource(renzora_shared::renzora_splash::PendingProjectReopen);

        if let Some(ref path) = project_path {
            log::info!("[ENGINE] Opening project: {}", path.display());
            let project_toml = path.join("project.toml");
            match renzora_shared::renzora_core::open_project(&project_toml) {
                Ok(project) => {
                    log::info!("[ENGINE] Project opened successfully");
                    app.insert_resource(project);
                }
                Err(e) => {
                    log::error!("[ENGINE] Failed to open project: {}", e);
                }
            }
        }
    }

    // Load core plugins (next to the exe)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    if let Some(ref dir) = exe_dir {
        let core_plugins = dir.join("plugins");
        if core_plugins.exists() {
            dynamic_plugin_loader::load_plugins(&mut app, &core_plugins, is_editor);
        }
    }

    // Load project plugins
    if let Some(ref dir) = plugins_dir {
        dynamic_plugin_loader::load_plugins(&mut app, dir, is_editor);
    }

    app.run();
}

// ── Splash screen ─────────────────────────────────────────────────────────

#[cfg(feature = "editor")]
fn run_splash() {
    use bevy::window::WindowPlugin;

    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(bevy::window::Window {
                title: "Renzora".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }),
    );
    app.add_plugins(renzora_shared::renzora_splash::SplashPlugin);
    app.add_systems(bevy::app::Startup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });

    app.add_systems(
        bevy::app::Update,
        |project: Option<Res<renzora_shared::renzora_core::CurrentProject>>| {
            if let Some(proj) = project {
                let exe = std::env::current_exe().expect("Failed to get exe path");
                let _ = std::process::Command::new(&exe)
                    .arg("--project")
                    .arg(&proj.path)
                    .spawn();
                std::process::exit(0);
            }
        },
    );

    app.run();
}

// ── Binary name detection ─────────────────────────────────────────────────

fn is_editor_binary() -> bool {
    let exe = std::env::current_exe().unwrap_or_default();
    let name = exe
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    // "renzora-runtime" or "renzora-game" → not editor
    // "renzora" → editor
    !name.contains("runtime") && !name.contains("game")
}
