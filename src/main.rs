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

/// Load community plugins from `<exe-dir>/plugins/`.
///
/// The editor is no longer loaded here. It used to arrive as a `dlopen`'d
/// `renzora_editor` cdylib sharing this binary's `bevy_dylib`; it is now a
/// separate executable (`crates/renzora_editor_app`) that links the editor
/// statically. With Bevy statically linked there is nothing for a loadable
/// bundle to share — a cdylib linking static Bevy would carry its own copy of
/// Bevy and therefore its own `World` type.
///
/// The consequence worth stating plainly: **this binary can no longer become
/// the editor under any circumstance.** It is always a game, a dedicated server
/// or a listen server, which is exactly what makes it safe to ship.
fn load_global_plugins(app: &mut App, is_editor: bool) {
    // C-ABI plugins from `<exe-dir>/plugins/`. The only plugin mechanism left:
    // the Bevy-linking `dlopen` path (and its `dynamic_plugin_loader`) is gone,
    // because a cdylib linking a statically-linked Bevy carries its own copy of
    // Bevy and therefore its own `World` type. The former distribution plugins
    // are now ordinary rlib dependencies of `renzora_runtime`.
    //
    // `is_editor` is the scope gate: a C-ABI plugin declares Runtime or Editor via
    // `renzora_plugin_scope`, read BEFORE its init is called, so an editor-only
    // panel plugin never activates in a shipped game and vice versa.
    app.add_plugins(renzora_plugin::host::loader::RenzoraPluginHostPlugin { is_editor });
    // Installs any render passes those plugins registered. Separate plugin
    // because the work happens in `finish`, after every `build` has run and the
    // render sub-app exists.
    app.add_plugins(renzora_postprocess::plugin_bridge::PluginRenderBridgePlugin);
    // Custom shaded materials registered by those plugins. Separate plugin: it
    // owns an asset type and a `MaterialPlugin`, and builds its assets in
    // `finish` for the same reason the render bridge does.
    renzora_postprocess::add_plugin_material(app);
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
    // Always false: this binary is the runtime. The editor is a separate
    // executable now (see `load_global_plugins`). Kept as a variable rather than
    // inlined because `is_editor` is threaded through `add_default_rendering` /
    // `add_engine_plugins`, which are shared with the editor binary.
    let is_editor = false;
    let _ = (server_mode, host_mode, vr_mode);

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
