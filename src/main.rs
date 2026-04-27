#![allow(unused_imports)]

use bevy::prelude::*;

// ── App setup helpers ────────────────────────────────────────────────────
//
// Most setup lives in `renzora_runtime` (the shared meta-crate). The two
// items below stay here because they are binary-level deployment decisions:
// `add_default_rendering` swaps in a no-window plugin set for the dedicated
// server, and `build_runtime_app` is the entry point WASM bindings call.

pub fn init_app() -> App {
    renzora_runtime::init_app()
}

pub fn add_engine_plugins(app: &mut App) {
    renzora_runtime::add_engine_plugins(app);
}

pub fn add_default_rendering(app: &mut App) {
    #[cfg(any(feature = "editor", not(feature = "server")))]
    renzora_runtime::add_default_rendering(app);

    #[cfg(all(feature = "server", not(feature = "editor")))]
    {
        app.add_plugins(
            DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
        );
    }
}

/// Build the full runtime app (used by WASM `start` and the dedicated server).
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}

/// Scan `<exe_dir>/plugins/` for dynamic plugins and load them. Called once
/// at startup, before `app.run()`. The plugin loader filters by scope so an
/// editor-scope plugin won't activate in a runtime build and vice versa.
fn load_global_plugins(app: &mut App, is_editor: bool) {
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    else {
        return;
    };
    let plugins = exe_dir.join("plugins");
    if plugins.exists() {
        dynamic_plugin_loader::load_plugins(app, &plugins, is_editor);
    }
}

// ── WASM runtime ─────────────────────────────────────────────────────────

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
fn main() {}

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_rpak(data: &[u8]) {
    renzora_runtime::renzora_engine::vfs::set_wasm_rpak(data.to_vec());
}

#[cfg(all(target_arch = "wasm32", feature = "runtime"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start() {
    let mut app = build_runtime_app();
    app.run();
}

// ── Native entry point ───────────────────────────────────────────────────

#[cfg(not(all(target_arch = "wasm32", feature = "runtime")))]
fn main() {
    renzora_runtime::renzora_engine::crash::install_panic_hook();

    // ── Editor ───────────────────────────────────────────────────────
    //
    // Single Bevy app with both splash and editor plugins. The splash UI
    // runs while `SplashState::Splash`, transitions through `Loading`, and
    // hands off to the editor UI on `SplashState::Editor`. No subprocess
    // spawn, no IPC, no sentinel.
    #[cfg(feature = "editor")]
    {
        let mut app = init_app();
        add_default_rendering(&mut app);
        add_engine_plugins(&mut app);
        app.add_plugins(renzora_runtime::renzora_engine::crash::CrashReportPlugin);
        renzora_runtime::add_editor_plugins(&mut app);

        // Optional `--project <path>` shortcut for dev workflows: skip the
        // splash UI and jump straight into the project. Splash plugin sees
        // `PendingProjectReopen` and immediately transitions to Loading.
        if let Some(project_path) = parse_project_arg() {
            log::info!("[ENGINE] --project arg: {}", project_path.display());
            let project_toml = project_path.join("project.toml");
            match renzora_runtime::renzora::open_project(&project_toml) {
                Ok(project) => {
                    app.insert_resource(project);
                    app.insert_resource(renzora_runtime::renzora_splash::PendingProjectReopen);
                }
                Err(e) => log::error!("[ENGINE] Failed to open project: {}", e),
            }
        }

        load_global_plugins(&mut app, true);
        app.run();
    }

    // ── Runtime ──────────────────────────────────────────────────────
    #[cfg(feature = "runtime")]
    {
        let mut app = init_app();
        add_default_rendering(&mut app);
        add_engine_plugins(&mut app);
        app.add_plugins(renzora_runtime::renzora_engine::crash::CrashReportPlugin);
        load_global_plugins(&mut app, false);
        app.run();
    }

    // ── Server ───────────────────────────────────────────────────────
    #[cfg(feature = "server")]
    {
        let mut app = build_runtime_app();
        app.add_plugins(renzora_runtime::renzora_engine::crash::CrashReportPlugin);

        let net_config = load_server_config();
        info!(
            "[server] Starting dedicated server on {}:{}",
            net_config.server_addr, net_config.port
        );
        app.add_plugins(renzora_runtime::renzora_network::NetworkServerPlugin::new(net_config));
        load_global_plugins(&mut app, false);
        app.run();
    }
}

#[cfg(all(feature = "editor", not(target_arch = "wasm32")))]
fn parse_project_arg() -> Option<std::path::PathBuf> {
    std::env::args()
        .skip_while(|a| a != "--project")
        .nth(1)
        .map(std::path::PathBuf::from)
}

// ── Server config ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
fn load_server_config() -> renzora_runtime::renzora_network::NetworkConfig {
    use renzora_runtime::renzora_network;
    use renzora_runtime::renzora;

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
