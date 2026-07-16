#![allow(unused_imports)]
// The desktop binary is always runtime-shaped: on Windows release it launches
// windowless so shipped games don't pop a console. Editor and server sessions
// grab a console at startup via `attach_console()`; a shipped game stays
// console-free unless `project.toml` opts in (`console_logging`). The editor
// experience is layered on at runtime by the editor bundle dll beside the exe.
#![cfg_attr(
    all(
        target_os = "windows",
        feature = "runtime",
        not(debug_assertions)
    ),
    windows_subsystem = "windows"
)]

use bevy::prelude::*;

// Force-link statically-embedded distribution plugins for a lean single-binary
// export. The `static_plugins` feature is OFF in every normal build; the lean
// exporter turns it on and regenerates the `renzora_static_plugins` aggregator
// to depend on the plugins a game uses. This `extern crate` keeps the
// aggregator — and through its own `extern crate` lines, each plugin's
// `inventory::submit!` ctor — from being dead-stripped, so the runtime's
// `for_each_static_plugin` loop discovers and installs them at boot (no dlopen).
#[cfg(feature = "static_plugins")]
extern crate renzora_static_plugins;

// ── App setup helpers ────────────────────────────────────────────────────
//
// Most setup lives in `renzora_runtime` (the shared meta-crate). The two
// items below stay here because they are binary-level deployment decisions:
// `add_default_rendering` installs the windowed client plugin set, and
// `build_runtime_app` is the entry point WASM bindings call. The dedicated
// server is no longer a separate binary — it's the runtime launched with
// `--server`, which swaps in a windowless plugin set inline in `main`.

pub fn init_app() -> App {
    renzora_runtime::init_app()
}

pub fn add_engine_plugins(app: &mut App, is_editor: bool) {
    renzora_runtime::add_engine_plugins(app, is_editor);
}

pub fn add_default_rendering(app: &mut App, is_editor: bool) {
    renzora_runtime::add_default_rendering(app, is_editor);
}

/// Build the full runtime app (used by WASM `start`). Always a game.
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app, false);
    add_engine_plugins(&mut app, false);
    app
}

/// Directory containing the running executable.
fn exe_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Path to the editor bundle cdylib sitting beside the exe, if present.
/// Cargo prefixes cdylibs with `lib` on Unix, so both stems are checked.
/// **Removing this one file is what turns the editor binary into a shipped
/// game** — the binary itself is identical either way.
fn editor_bundle_path() -> Option<std::path::PathBuf> {
    let dir = exe_dir()?;
    #[cfg(target_os = "windows")]
    let names: &[&str] = &["renzora_editor.dll"];
    #[cfg(target_os = "linux")]
    let names: &[&str] = &["librenzora_editor.so", "renzora_editor.so"];
    #[cfg(target_os = "macos")]
    let names: &[&str] = &["librenzora_editor.dylib", "renzora_editor.dylib"];
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let names: &[&str] = &[];
    names.iter().map(|n| dir.join(n)).find(|p| p.exists())
}

/// Whether this launch is an editor session: the editor bundle is present
/// beside the exe and no override forces game mode (`--no-editor` /
/// `RENZORA_NO_EDITOR`). Server/host launches are excluded by the caller.
fn editor_session() -> bool {
    if std::env::args().any(|a| a == "--no-editor")
        || std::env::var_os("RENZORA_NO_EDITOR").is_some()
    {
        return false;
    }
    editor_bundle_path().is_some()
}

/// Load the editor bundle (editor sessions only) plus any community plugins
/// from `<exe_dir>/plugins/`. Called once at startup, AFTER
/// `add_engine_plugins`, so the bundle's Editor-scope plugins layer on top of
/// the runtime foundation — reproducing the old `add_editor_plugins` ordering.
/// The directory loader filters by scope so an editor-scope community plugin
/// won't activate in a game and vice versa.
fn load_global_plugins(app: &mut App, is_editor: bool) {
    if is_editor {
        if let Some(bundle) = editor_bundle_path() {
            dynamic_plugin_loader::load_bundle(app, &bundle, true);
        }
    }
    if let Some(dir) = exe_dir() {
        let plugins = dir.join("plugins");
        if plugins.exists() {
            dynamic_plugin_loader::load_plugins(app, &plugins, is_editor);
        }
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

// One binary, three runtime-decided modes:
//   editor    : the editor bundle dll is present beside the exe (default dev
//               build). The runtime app boots; `load_global_plugins` dlopens
//               the bundle, which layers the splash + editor plugins on top.
//   game      : no bundle (or `--no-editor`). The same binary runs as the
//               exported game — windowed client, OS title bar.
//   server    : `--server` (headless, no GPU) or `--host` (windowed listen
//               server). Never an editor session.
// The single binary IS the exported game; removing the editor bundle is the
// only difference between shipping the editor and shipping the game.
#[cfg(not(all(target_arch = "wasm32", feature = "runtime")))]
fn main() {
    // `--host` wins if both are passed. A server/host launch is never an
    // editor session even if the bundle dll happens to sit beside the exe.
    let host_mode = std::env::args().any(|a| a == "--host");
    let server_mode = !host_mode && std::env::args().any(|a| a == "--server");
    // `--vr` boots the game into the headset (OpenXR owns render init, so the
    // decision must be made here, before plugins assemble). VR implies game
    // mode — the editor never runs in-headset; its "VR Headset" play target
    // launches this exact flag on a child process. Ignored for server/host.
    let vr_mode =
        !host_mode && !server_mode && std::env::args().any(|a| a == "--vr");
    let is_editor = !server_mode && !host_mode && !vr_mode && editor_session();

    // Install the panic hook now that we know the session kind — it picks the
    // crash-file location + dialog from `is_editor` (it can't read the World).
    renzora_runtime::renzora_engine::crash::install_panic_hook(is_editor);

    // Windows release is `windows_subsystem = "windows"` (no console). Editor
    // sessions grab one so their log output is visible; a shipped game stays
    // console-free unless `project.toml` opts in. (The dedicated server grabs
    // its own below.)
    if is_editor {
        renzora_runtime::attach_console();
    }

    let mut app = init_app();

    // Load the network config up front so the headless runner and the network
    // server plugin share one tick rate.
    let server_config = (server_mode || host_mode).then(load_server_config);

    if let Some(net_config) = &server_config {
        if host_mode {
            // Host/listen-server: windowed client + server in one process.
            // Mark host mode before engine plugins build so NetworkPlugin wires
            // the client half and lets the server plugin own the protocol. The
            // host renders, so it is NOT headless (and is never the editor).
            app.init_resource::<renzora_runtime::renzora::HostServer>();
            add_default_rendering(&mut app, false);
        } else {
            // Dedicated server: grab a console for its log output, then boot
            // headless — no GPU, no window, no winit — driven by a fixed-rate
            // runner at the network tick. See `add_headless_rendering`.
            renzora_runtime::attach_console();
            app.init_resource::<renzora_runtime::renzora::DedicatedServer>();
            renzora_runtime::add_headless_rendering(&mut app, net_config.tick_rate);
        }
    } else if vr_mode {
        #[cfg(feature = "xr")]
        {
            // VR sessions keep a console: OpenXR runtime discovery failures
            // (no headset, runtime not installed) surface as log lines that
            // would otherwise vanish with the windowless subsystem.
            renzora_runtime::attach_console();
            renzora_runtime::add_xr_rendering(&mut app);
        }
        #[cfg(not(feature = "xr"))]
        {
            eprintln!(
                "--vr requested but this build has no XR support (built without \
                 the `xr` feature); starting flat."
            );
            add_default_rendering(&mut app, is_editor);
        }
    } else {
        add_default_rendering(&mut app, is_editor);
    }

    add_engine_plugins(&mut app, is_editor);
    app.add_plugins(renzora_runtime::renzora_engine::crash::CrashReportPlugin);

    if let Some(net_config) = server_config {
        info!(
            "[server] Starting {} on {}:{}",
            if host_mode { "host server" } else { "dedicated server" },
            net_config.server_addr,
            net_config.port
        );
        app.add_plugins(renzora_runtime::renzora_network::NetworkServerPlugin::new(
            net_config,
        ));
    }

    // Editor bundle (editor sessions) + community plugins, after the engine
    // foundation. The `--project <path>` dev shortcut moved into the splash
    // plugin (it lives in the bundle now).
    load_global_plugins(&mut app, is_editor);

    // Watch `plugins/` for dlls dropped in mid-session and hot-load them into
    // the live world (main-world plugins activate next frame; render plugins
    // toast "restart to take effect"). No-op on platforms without dynamic
    // linking.
    if let Some(dir) = exe_dir() {
        app.add_plugins(dynamic_plugin_loader::HotPluginPlugin::new(
            dir.join("plugins"),
            is_editor,
        ));
    }

    app.run();
}

// ── Server config ────────────────────────────────────────────────────────

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
fn load_server_config() -> renzora_runtime::renzora_network::NetworkConfig {
    use renzora_runtime::renzora;
    use renzora_runtime::renzora_network;

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
