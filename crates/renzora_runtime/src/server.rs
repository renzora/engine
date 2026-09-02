//! Dedicated-server and host-server startup, behind the `networking` feature.
//!
//! # Why this is here and not in `src/main.rs`
//!
//! It used to be there, and that is what kept `renzora_network` out of the lean
//! exporter's reach: the binary read `renzora_runtime::renzora_network` directly,
//! so making the dependency optional would have broken `main.rs`, and the
//! manifest carried a note saying so. Multiplayer was therefore compiled into
//! every exported game, single-player ones included — one of the last engine
//! subsystems with no toggle at all.
//!
//! Moving it means the `#[cfg]` sits in the crate that owns the feature, and the
//! binary talks to an [`ServerConfig`] that exists either way. `main.rs` keeps
//! its shape; with `networking` off, [`config_from_args`] simply answers `None`
//! and the process boots as an ordinary client.
//!
//! [`ServerConfig`] is deliberately opaque. The binary needs three things from
//! it — a tick rate for the headless runner, a line to log, and the ability to
//! hand it back — and nothing else, so nothing else crosses the boundary and the
//! stub below has only three methods to imitate.

use bevy::prelude::App;

#[cfg(not(all(feature = "networking", not(target_arch = "wasm32"))))]
pub use disabled::*;
#[cfg(all(feature = "networking", not(target_arch = "wasm32")))]
pub use enabled::*;

#[cfg(all(feature = "networking", not(target_arch = "wasm32")))]
mod enabled {
    use super::App;
    use renzora_network::NetworkConfig;

    /// A resolved server configuration: command-line flags over `project.toml`'s
    /// `[network]` section over the built-in defaults.
    pub struct ServerConfig(NetworkConfig);

    impl ServerConfig {
        /// Ticks per second the headless runner should drive the app at, so the
        /// simulation and the network loop share one rate.
        pub fn tick_rate(&self) -> u16 {
            self.0.tick_rate
        }

        /// `address:port`, for the startup log line.
        pub fn endpoint(&self) -> String {
            format!("{}:{}", self.0.server_addr, self.0.port)
        }
    }

    /// Read the server configuration, or `None` when this build has no
    /// networking. Only called when `--server` or `--host` was passed.
    pub fn config_from_args() -> Option<ServerConfig> {
        let mut config = NetworkConfig::default();
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

        // `project.toml` fills in whatever the command line did not say, so a
        // flag always wins over the file rather than the other way round.
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

        Some(ServerConfig(config))
    }

    /// Add the server half. The host case adds this *alongside* the ordinary
    /// client `NetworkPlugin`; a dedicated server adds only this.
    pub fn install(app: &mut App, config: ServerConfig) {
        app.add_plugins(renzora_network::NetworkServerPlugin::new(config.0));
    }
}

#[cfg(not(all(feature = "networking", not(target_arch = "wasm32"))))]
mod disabled {
    use super::App;

    /// Stand-in with the same shape, so the binary compiles unchanged. It is
    /// never constructed: [`config_from_args`] is the only source and it always
    /// answers `None`.
    pub struct ServerConfig;

    impl ServerConfig {
        pub fn tick_rate(&self) -> u16 {
            60
        }
        pub fn endpoint(&self) -> String {
            String::new()
        }
    }

    pub fn config_from_args() -> Option<ServerConfig> {
        None
    }

    pub fn install(_app: &mut App, _config: ServerConfig) {}
}
