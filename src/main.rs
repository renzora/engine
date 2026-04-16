#![allow(unused_imports)]

use bevy::prelude::*;
use renzora_app::{renzora_shared, init_app, add_default_rendering, add_engine_plugins};

// ── WASM runtime ─────────────────────────────────────────────────────────

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
fn main() {}

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_rpak(data: &[u8]) {
    renzora_shared::renzora_engine::vfs::set_wasm_rpak(data.to_vec());
}

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start() {
    let mut app = renzora_app::build_runtime_app();
    app.run();
}

// ── Native entry point ───────────────────────────────────────────────────

#[cfg(not(all(target_arch = "wasm32", feature = "runtime")))]
fn main() {
    renzora_shared::renzora_engine::crash::install_panic_hook();

    // ── Editor ───────────────────────────────────────────────────────
    #[cfg(feature = "editor")]
    {
        let project_path = std::env::args()
            .skip_while(|a| a != "--project")
            .nth(1)
            .map(std::path::PathBuf::from);

        if let Some(ref p) = project_path {
            log::info!("[ENGINE] --project arg: {:?}", p);
        }

        // No --project arg = show splash, pick project, restart with --project
        if project_path.is_none() {
            run_splash();
            return;
        }

        // Project-wide plugin scan: any dylib anywhere in the project is picked
        // up. Users can place a plugin alongside its assets (marketplace bundle
        // pattern) or in a dedicated plugins/ folder — both work.
        let plugins_dir = project_path.as_ref().cloned();

        let mut app = init_app();
        add_default_rendering(&mut app);
        add_engine_plugins(&mut app);
        app.add_plugins(renzora_shared::renzora_engine::crash::CrashReportPlugin);

        // Core editor infrastructure (must load before dynamic plugins)
        app.add_plugins(renzora_shared::renzora_undo::UndoPlugin);
        app.add_plugins(renzora_shared::renzora_splash::SplashPlugin);
        app.add_plugins(renzora_shared::renzora_editor_framework::RenzoraEditorPlugin);
        app.add_plugins(renzora_shared::renzora_grid::GridPlugin);
        app.add_plugins(renzora_shared::renzora_keybindings::KeybindingsPlugin);
        app.add_plugins(renzora_shared::renzora_viewport::ViewportPlugin);
        app.add_plugins(renzora_shared::renzora_camera::CameraPlugin);
        app.add_plugins(renzora_shared::renzora_gizmo::GizmoPlugin);
        app.add_plugins(renzora_shared::renzora_scene::ScenePlugin);
        app.add_plugins(renzora_shared::renzora_console::ConsolePlugin);

        app.insert_resource(renzora_shared::renzora_splash::PendingProjectReopen);

        if let Some(ref path) = project_path {
            log::info!("[ENGINE] Opening project: {}", path.display());
            let project_toml = path.join("project.toml");
            match renzora_shared::renzora::open_project(&project_toml) {
                Ok(project) => {
                    log::info!("[ENGINE] Project opened successfully");
                    app.insert_resource(project);
                }
                Err(e) => {
                    log::error!("[ENGINE] Failed to open project: {}", e);
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

        // Load project plugins (recursive — dylibs can live anywhere in the
        // game project, alongside their prefabs/assets).
        if let Some(ref dir) = plugins_dir {
            dynamic_plugin_loader::load_plugins_recursive(&mut app, dir, true);
        }

        app.run();
    }

    // ── Runtime ──────────────────────────────────────────────────────
    #[cfg(feature = "runtime")]
    {
        let mut app = init_app();
        add_default_rendering(&mut app);
        add_engine_plugins(&mut app);
        app.add_plugins(renzora_shared::renzora_engine::crash::CrashReportPlugin);

        // Load plugins from plugins/ next to the exe
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        if let Some(ref dir) = exe_dir {
            let plugins = dir.join("plugins");
            if plugins.exists() {
                dynamic_plugin_loader::load_plugins(&mut app, &plugins, false);
            }
        }

        app.run();
    }

    // ── Server ───────────────────────────────────────────────────────
    #[cfg(feature = "server")]
    {
        let mut app = renzora_app::build_runtime_app();
        app.add_plugins(renzora_shared::renzora_engine::crash::CrashReportPlugin);

        let net_config = load_server_config();
        info!(
            "[server] Starting dedicated server on {}:{}",
            net_config.server_addr, net_config.port
        );
        app.add_plugins(renzora_shared::renzora_network::NetworkServerPlugin::new(net_config));
        app.run();
    }
}

// ── Splash screen ────────────────────────────────────────────────────────

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
        |project: Option<Res<renzora_shared::renzora::CurrentProject>>| {
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

// ── Server config ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
fn load_server_config() -> renzora_shared::renzora_network::NetworkConfig {
    use renzora_shared::renzora_network;
    use renzora_shared::renzora;

    let mut config = renzora_network::NetworkConfig::default();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(port) = val.parse::<u16>() {
                        config.port = port;
                    }
                    i += 1;
                }
            }
            "--addr" | "--address" => {
                if let Some(val) = args.get(i + 1) {
                    config.server_addr = val.clone();
                    i += 1;
                }
            }
            "--tick-rate" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(rate) = val.parse::<u16>() {
                        config.tick_rate = rate;
                    }
                    i += 1;
                }
            }
            "--max-clients" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(max) = val.parse::<u16>() {
                        config.max_clients = max;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let project_toml = std::path::PathBuf::from("project.toml");
    if project_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&project_toml) {
            if let Ok(project_config) = toml::from_str::<renzora::ProjectConfig>(&content) {
                if let Some(net) = &project_config.network {
                    if !args.iter().any(|a| a == "--port") {
                        config.port = net.port;
                    }
                    if !args.iter().any(|a| a == "--addr" || a == "--address") {
                        config.server_addr = net.server_addr.clone();
                    }
                    if !args.iter().any(|a| a == "--tick-rate") {
                        config.tick_rate = net.tick_rate;
                    }
                    if !args.iter().any(|a| a == "--max-clients") {
                        config.max_clients = net.max_clients;
                    }
                    config.transport =
                        renzora_network::TransportKind::from_str_loose(&net.transport);
                }
            }
        }
    }

    config
}
