// Use the editor dylib (superset) or runtime dylib depending on build.
#[cfg(feature = "editor")]
use renzora_editor as renzora_shared;
#[cfg(not(feature = "editor"))]
use renzora_runtime as renzora_shared;

// On WASM runtime (no editor), main() is a no-op — JS calls set_rpak() then start().
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

#[cfg(not(all(target_arch = "wasm32", not(feature = "editor"))))]
fn main() {
    renzora_shared::renzora_engine::crash::install_panic_hook();

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
    if project_path.is_none() {
        run_splash();
        return; // splash handles restart, we exit here
    }

    // Determine plugins directory
    let plugins_dir = project_path.as_ref().map(|p| p.join("plugins"));

    // Phase 1: Create app base (asset reader, DLSS)
    let mut app = renzora_app::init_app();

    // Phase 2: Check for XR plugin and let it set up rendering, or use default
    let xr_mode = plugins_dir.as_ref()
        .map(|dir| dynamic_plugin_loader::try_init_xr_rendering(&mut app, dir))
        .unwrap_or(false);

    if !xr_mode {
        renzora_app::add_default_rendering(&mut app);
    }

    // Phase 3: Add engine plugins
    renzora_app::add_engine_plugins(&mut app);

    app.add_plugins(renzora_shared::renzora_engine::crash::CrashReportPlugin);

    #[cfg(feature = "editor")] {
        // Core editor infrastructure (must load before dynamic plugins)
        app.add_plugins(renzora_editor::renzora_splash::SplashPlugin);
        app.add_plugins(renzora_editor::renzora_editor_framework::RenzoraEditorPlugin);
        app.add_plugins(renzora_editor::renzora_grid::GridPlugin);
        app.add_plugins(renzora_editor::renzora_keybindings::KeybindingsPlugin);
        app.add_plugins(renzora_editor::renzora_viewport::ViewportPlugin);
        app.add_plugins(renzora_editor::renzora_camera::CameraPlugin);
        app.add_plugins(renzora_editor::renzora_gizmo::GizmoPlugin);
        app.add_plugins(renzora_editor::renzora_scene::ScenePlugin);

        // All other panels are standalone plugin DLLs loaded from plugins/

        // Skip splash — go straight to editor
        app.insert_resource(renzora_editor::renzora_splash::PendingProjectReopen);

        // Insert the project
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
            dynamic_plugin_loader::load_plugins(&mut app, &core_plugins, true);
        }
    }

    // Load project plugins (from the project's plugins directory)
    if let Some(ref dir) = plugins_dir {
        dynamic_plugin_loader::load_plugins(&mut app, dir, true);
    }

    app.run();
}

/// Splash screen — shows project picker, then restarts process with --project arg.
#[cfg(feature = "editor")]
fn run_splash() {
    use bevy::prelude::*;
    use bevy::window::WindowPlugin;

    let mut app = App::new();

    app.insert_resource(bevy::anti_alias::dlss::DlssProjectId(
        uuid::Uuid::from_bytes([
            0x72, 0x65, 0x6e, 0x7a, 0x6f, 0x72, 0x61, 0x2d,
            0x65, 0x6e, 0x67, 0x69, 0x6e, 0x65, 0x30, 0x31,
        ]),
    ));

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
    app.add_plugins(renzora_editor::renzora_splash::SplashPlugin);
    app.add_systems(bevy::app::Startup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });

    // Watch for project selection, then restart with --project
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
