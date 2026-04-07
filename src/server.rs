//! Renzora dedicated server binary.
//!
//! Headless: no window, no renderer, no GPU, no audio, no postprocessing.
//! Runs authoritative physics + scripting + networking.
//! Deploys to any Linux/Windows VPS.

use bevy::prelude::*;

#[cfg(feature = "editor")]
use renzora_editor as renzora_shared;
#[cfg(not(feature = "editor"))]
use renzora_runtime as renzora_shared;

use renzora_shared::renzora_network;
use renzora_shared::renzora_core;

fn main() {
    let mut app = renzora_app::build_runtime_app();

    // Load network config from project/CLI and start the server
    let net_config = load_server_config();
    info!(
        "[server] Starting dedicated server on {}:{}",
        net_config.server_addr, net_config.port
    );

    app.add_plugins(renzora_network::NetworkServerPlugin::new(net_config));

    app.run();
}

/// Load network configuration from project config or CLI args.
fn load_server_config() -> renzora_network::NetworkConfig {
    let mut config = renzora_network::NetworkConfig::default();

    // Parse CLI args
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

    // Override with project config if available
    let project_toml = std::path::PathBuf::from("project.toml");
    if project_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&project_toml) {
            if let Ok(project_config) = toml::from_str::<renzora_core::ProjectConfig>(&content) {
                if let Some(net) = &project_config.network {
                    // CLI args take precedence, so only fill in defaults
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
